import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from verification import ROOT, run_checks, summarize, validate_receipts


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / filename)
    result = importlib.util.module_from_spec(spec); spec.loader.exec_module(result)
    return result


publisher = module('publisher', 'publish-final-evidence.py')
pr = module('pr', 'pr-body.py')
snapshot = module('snapshot', 'verify-source-snapshot.py')
report_index = module('report_index', 'report-index.py')
capture = module('capture', 'capture-runtime-diagnostics.py')
checker = module('checker', 'check.py')

# A stand-in for Playwright: the driver only needs a page that fails a
# navigation, so the failure-evidence path can be exercised without a browser.
FAKE_PLAYWRIGHT = """
const fs = require('node:fs');
const path = require('node:path');
function makePage() {
  const handlers = {};
  const page = {
    lastUrl: 'about:blank',
    on(event, handler) { (handlers[event] = handlers[event] || []).push(handler); },
    async goto(url) {
      page.lastUrl = url;
      for (const handler of handlers.console || []) handler({type: () => 'error', text: () => 'fake console error'});
      throw new Error('fake navigation failed for ' + url);
    },
    url() { return page.lastUrl; },
    async screenshot(options) {
      fs.mkdirSync(path.dirname(options.path), {recursive: true});
      fs.writeFileSync(options.path, 'fake-png');
    },
    context() { return {pages: () => [page]}; },
  };
  return page;
}
module.exports = {chromium: {
  executablePath: () => '/fake/chromium',
  async launch() {
    return {version: () => 'fake-browser-1.0', newPage: async () => makePage(), close: async () => {}};
  },
}};
"""


class VerificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)

    def run_case(self, exit_code=0):
        raw = self.base / 'raw'
        status = run_checks([('example', [sys.executable, '-c', f'print("test result: ok. 2 passed; 0 failed; 1 ignored;"); raise SystemExit({exit_code})'])], raw)
        return raw, status

    def failing_run(self, name='failed'):
        """A run whose first check fails and whose second check still runs."""
        raw = self.base / name
        status = run_checks([('failure', [sys.executable, '-c', 'raise SystemExit(3)']),
                             ('later', [sys.executable, '-c', 'print("test result: ok. 1 passed; 0 failed; 0 ignored;")'])], raw)
        return raw, status

    def test_failed_run_records_every_check_and_its_outcome(self):
        raw, status = self.failing_run()
        self.assertEqual(status, 3)
        manifest = json.loads((raw / 'manifest.json').read_text())
        self.assertTrue(manifest['completed'])
        self.assertEqual(manifest['outcome'], 'failed')
        self.assertEqual(manifest['failed_checks'], ['failure'])
        self.assertEqual([c['name'] for c in manifest['checks']], ['failure', 'later'])
        self.assertEqual([c['status'] for c in manifest['checks']], ['failed', 'passed'])
        self.assertTrue((raw / 'later.log').exists())

    def test_successful_run_reports_a_passing_outcome(self):
        raw, status = self.run_case(); self.assertEqual(status, 0)
        manifest = json.loads((raw / 'manifest.json').read_text())
        self.assertEqual(manifest['outcome'], 'passed')
        self.assertEqual(manifest['failed_checks'], [])
        self.assertTrue(manifest['completed'])

    def test_manifest_records_environment_and_ci(self):
        raw, _ = self.run_case()
        manifest = json.loads((raw / 'manifest.json').read_text())
        self.assertEqual(set(manifest['environment']),
                         {'os', 'arch', 'platform', 'python', 'rustc', 'cargo', 'node', 'browser', 'env_names'})
        self.assertEqual(set(manifest['ci']), {'run_id', 'run_url', 'workflow', 'repository', 'sha', 'ref'})
        self.assertIsInstance(manifest['environment']['env_names'], list)
        for name in manifest['environment']['env_names']:
            self.assertIsInstance(name, str)

    def test_failed_receipt_is_refused_outside_the_failures_directory(self):
        raw, _ = self.failing_run()
        with self.assertRaises(ValueError): publisher.publish(raw, self.base / 'published')
        self.assertFalse((self.base / 'published').exists())

    def test_failed_receipt_publishes_under_failures(self):
        raw, _ = self.failing_run()
        output = self.base / 'failures' / '2026-10-21-example' / 'checks'
        publisher.publish(raw, output)
        self.assertTrue((output / 'failure.log').is_file())
        manifest = json.loads((output / 'manifest.json').read_text())
        self.assertEqual(manifest['outcome'], 'failed')
        self.assertEqual(manifest['failed_checks'], ['failure'])

    def test_successful_receipt_is_refused_under_failures(self):
        raw, _ = self.run_case()
        output = self.base / 'failures' / '2026-10-21-example' / 'checks'
        with self.assertRaises(ValueError): publisher.publish(raw, output)
        self.assertFalse(output.exists())

    def test_success_preserves_actual_receipts_and_refuses_overwrite(self):
        raw, status = self.run_case(); self.assertEqual(status, 0)
        manifest = validate_receipts(raw)
        self.assertEqual(manifest['checks'][0]['summary']['rust_totals'], {'passed': 2, 'failed': 0, 'ignored': 1})
        compact = json.loads((raw / 'review-summary.json').read_text())
        self.assertNotIn('test_results', compact['checks'][0]['summary'])
        output = self.base / 'published'; publisher.publish(raw, output)
        self.assertEqual(json.loads((output / 'manifest.json').read_text())['checks'][0]['exit_code'], 0)
        self.assertNotIn('test_results', json.loads((output / 'review-summary.json').read_text())['checks'][0]['summary'])
        with self.assertRaises(ValueError): publisher.publish(raw, output)

    def test_missing_or_tampered_receipts_rejected(self):
        raw, _ = self.run_case()
        (raw / 'example.log').write_text('changed')
        with self.assertRaises(ValueError): validate_receipts(raw)
        (raw / 'example.log').unlink()
        with self.assertRaises(FileNotFoundError): validate_receipts(raw)

    def test_tampered_artifact_rejected(self):
        raw = self.base / 'raw'
        script = 'import os,sys; open(os.path.join(sys.argv[1],"evidence.txt"),"w").write("payload")'
        run_checks([('example', [sys.executable, '-c', script, str(raw / 'example.artifacts')])], raw)
        manifest = json.loads((raw / 'manifest.json').read_text())
        self.assertEqual([a['path'] for a in manifest['artifacts']], ['example.artifacts/evidence.txt'])
        (raw / 'example.artifacts' / 'evidence.txt').write_text('tampered')
        with self.assertRaises(ValueError): validate_receipts(raw)

    def test_artifacts_are_published_with_both_digests(self):
        raw = self.base / 'raw'
        script = 'import os,sys; open(os.path.join(sys.argv[1],"evidence.txt"),"w").write("payload")'
        run_checks([('example', [sys.executable, '-c', script, str(raw / 'example.artifacts')])], raw)
        output = self.base / 'published'; publisher.publish(raw, output)
        copied = output / 'example.artifacts' / 'evidence.txt'
        self.assertTrue(copied.is_file())
        artifact = json.loads((output / 'manifest.json').read_text())['artifacts'][0]
        self.assertIn('raw_sha256', artifact)
        self.assertIn('published_sha256', artifact)
        self.assertEqual(artifact['raw_sha256'], artifact['published_sha256'])

    def test_incomplete_manifest_rejected(self):
        raw, _ = self.run_case()
        p = raw / 'manifest.json'; x = json.loads(p.read_text()); x['completed'] = False; p.write_text(json.dumps(x))
        with self.assertRaises(ValueError): validate_receipts(raw)

    def test_summary_adds_groups_instead_of_counting_named_tests_twice(self):
        result = summarize('test example ... ok\ntest result: ok. 6 passed; 0 failed; 0 ignored;\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n')
        self.assertEqual(result['rust_totals']['passed'], 7)
        self.assertEqual(result['rust_result_groups'], 2)
        self.assertEqual(summarize('Ran 4 tests in 0.2s\n\nOK\n')['python_result']['tests'], 4)

    def test_derived_check_needs_no_raw_data(self):
        result = subprocess.run([sys.executable, 'scripts/verify-source-snapshot.py', '--derived-only', '--data-dir', str(self.base / 'absent')], cwd=ROOT, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_pr_helper_preserves_suffix_byte_for_byte(self):
        suffix = pr.MARKER + '\n<h3>PR Summary by Qodo</h3>\nlarge appendix\n'
        self.assertEqual(pr.split_body('old\n\n' + suffix), ('old\n\n', suffix))
        self.assertEqual(pr.compose('old\n\n' + suffix, 'new\n'), 'new\n\n' + suffix)
        self.assertEqual(pr.compose('old', 'new'), 'new\n')
        with self.assertRaises(ValueError): pr.split_body('unexpected PR Summary by Qodo')

    def test_non_utf8_log_still_publishes(self):
        raw = self.base / 'raw'
        code = 'import sys; sys.stdout.buffer.write(b"ok \\xff\\n")'
        self.assertEqual(run_checks([('bytes', [sys.executable, '-c', code])], raw), 0)
        output = self.base / 'published'; publisher.publish(raw, output)
        self.assertIn('ok \ufffd', (output / 'bytes.log').read_text())

    def test_source_page_set_must_be_exact_and_distinct(self):
        pages = [{'page': '%s_%d' % page} for page in snapshot.PAGES]
        snapshot.assert_page_set(pages)
        duplicated = pages[:-1] + [pages[0]]
        with self.assertRaises(AssertionError): snapshot.assert_page_set(duplicated)
        with self.assertRaises(AssertionError): snapshot.assert_page_set(pages[:-1])

    def test_report_index_is_reproducible(self):
        reports = self.base / 'reports'
        (reports / 'run-a' / 'checks').mkdir(parents=True)
        manifest = {'schema_version': 1, 'profile': 'portable', 'source_commit': 'a' * 40, 'outcome': 'passed',
                    'started_at': '2026-10-21T00:00:00+00:00', 'finished_at': '2026-10-21T00:10:00+00:00',
                    'checks': [{'name': 'one', 'exit_code': 0}]}
        (reports / 'run-a' / 'checks' / 'manifest.json').write_text(json.dumps(manifest))
        first = report_index.regenerate(reports)
        second = report_index.regenerate(reports)
        self.assertEqual(first, second)
        rows = report_index.collect(reports)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['profile'], 'portable')
        self.assertEqual(rows[0]['outcome'], 'passed')
        self.assertEqual(rows[0]['date'], '2026-10-21')
        self.assertIn('run-a/checks/manifest.json', rows[0]['path'])
        self.assertEqual(report_index.check(reports), 0)
        (reports / 'index.json').write_text('{"schema_version": 1, "runs": []}\n')
        self.assertEqual(report_index.check(reports), 1)

    def test_report_index_reads_the_older_commands_receipts(self):
        reports = self.base / 'reports'
        (reports / 'final-verification').mkdir(parents=True)
        legacy = {'source_base_commit': 'b' * 40,
                  'checks': [{'order': 1, 'command': 'cargo test', 'result': 'exit 0', 'log': 'logs/1.log'}]}
        (reports / 'final-verification' / 'commands-and-hashes.json').write_text(json.dumps(legacy))
        rows = report_index.collect(reports)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['kind'], 'commands-and-hashes')
        self.assertEqual(rows[0]['source_commit'], 'b' * 40)
        self.assertEqual(rows[0]['outcome'], 'passed')

    def test_committed_report_index_matches_the_receipts(self):
        result = subprocess.run([sys.executable, 'scripts/report-index.py', '--check'], cwd=ROOT, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_failed_run_publishes_its_artifacts(self):
        raw = self.base / 'raw'
        script = 'import os,sys; open(os.path.join(sys.argv[1],"evidence.txt"),"w").write("payload")'
        run_checks([('failure', [sys.executable, '-c', script, str(raw / 'failure.artifacts')]),
                    ('later', [sys.executable, '-c', 'raise SystemExit(4)'])], raw)
        output = self.base / 'failures' / '2026-10-21-example' / 'checks'
        publisher.publish(raw, output)
        self.assertTrue((output / 'failure.artifacts' / 'evidence.txt').is_file())
        self.assertTrue((output / 'later.log').is_file())
        artifact = json.loads((output / 'manifest.json').read_text())['artifacts'][0]
        self.assertEqual(artifact['raw_sha256'], artifact['published_sha256'])

    def test_check_gives_every_check_an_artifact_directory(self):
        raw = self.base / 'raw'
        script = ('import os,sys; open(os.path.join(os.environ["NBA_CHECK_ARTIFACTS"],"note.txt"),"w").write("x")')
        run_checks([('example', [sys.executable, '-c', script])], raw)
        manifest = json.loads((raw / 'manifest.json').read_text())
        self.assertEqual([a['path'] for a in manifest['artifacts']], ['example.artifacts/note.txt'])
        self.assertEqual(manifest['checks'][0]['artifacts'], ['example.artifacts/note.txt'])

    def test_diagnostics_contract_is_enforced(self):
        body = {
            'build': {'commit': 'a' * 40, 'dirty': True, 'mode': 'reports'},
            'requests': {'total': 3, 'recent': [{'path': '/api/stats'}]},
            'semantic': {'state': 'unconfigured', 'events': [{'ts': 'now', 'outcome': 'unavailable',
                                                              'failure': 'unconfigured', 'elapsed_ms': 1.0}]},
            'snapshot': {'players': 5106, 'edges': 101395, 'sources': [], 'assets': []},
        }
        self.assertEqual(capture.validate_diagnostics(body), body)
        for broken, reason in [
            ({**body, 'build': {**body['build'], 'commit': 'unknown'}}, 'commit'),
            ({**body, 'build': {**body['build'], 'dirty': 'yes'}}, 'dirty'),
            ({**body, 'semantic': {'state': 'odd', 'events': []}}, 'state'),
            ({**body, 'semantic': {'state': 'unconfigured', 'events': []}}, 'event'),
            ({**body, 'requests': {'total': 1, 'recent': [{'path': '/api/query?q=secret'}]}}, 'query string'),
            ({**body, 'snapshot': {**body['snapshot'], 'players': 0}}, 'players'),
        ]:
            with self.assertRaises(SystemExit) as failure:
                capture.validate_diagnostics(broken)
            self.assertIn(reason, str(failure.exception))

    def test_semantic_events_must_not_carry_provider_receipts(self):
        body = {'build': {'commit': 'a' * 40, 'dirty': False, 'mode': 'reports'},
                'requests': {'total': 1, 'recent': []},
                'semantic': {'state': 'available', 'events': [{'ts': 'now', 'outcome': 'available',
                                                              'failure': None, 'elapsed_ms': 1.0,
                                                              'model': 'gpt', 'request_id': 'r-1'}]},
                'snapshot': {'players': 1, 'edges': 0, 'sources': [], 'assets': []}}
        with self.assertRaises(SystemExit) as failure:
            capture.validate_diagnostics(body)
        self.assertIn('model', str(failure.exception))

    def test_request_log_lines_must_be_json_objects(self):
        self.assertEqual(capture.validate_request_log('{"a": 1}\n\n{"b": 2}\n'), ['{"a": 1}', '{"b": 2}'])
        with self.assertRaises(SystemExit):
            capture.validate_request_log('')
        with self.assertRaises(SystemExit):
            capture.validate_request_log('not json\n')

    def test_check_profile_wires_the_diagnostics_and_index_checks(self):
        portable = dict(checker.commands('portable', self.base))
        self.assertIn('runtime-diagnostics', portable)
        self.assertIn('report-index', portable)
        self.assertIn('{artifacts}', ' '.join(portable['runtime-diagnostics']))
        browser = {name: command for name, *command in checker.commands('browser', self.base)}
        self.assertEqual(browser['browser-matrix'][1]['BROWSER_E2E_ARTIFACTS'], '{artifacts}')

    def test_browser_driver_writes_failure_evidence_into_the_artifact_directory(self):
        """The driver keeps its stdout diagnostic and files the evidence."""
        resolved = subprocess.run(['node', '-e', "console.log(require.resolve('playwright'))"],
                                   cwd=ROOT, capture_output=True, text=True)
        if resolved.returncode == 0:
            self.skipTest('a real playwright is installed; the browser profile covers that path')
        artifacts = self.base / 'artifacts'
        fake = self.base / 'fake-playwright.js'
        fake.write_text(FAKE_PLAYWRIGHT)
        secret = 'sk-live-do-not-print'
        environment = {**os.environ, 'PLAYWRIGHT_PACKAGE_PATH': str(fake),
                       'BROWSER_EXECUTABLE': '/fake/chromium',
                       'BROWSER_E2E_ARTIFACTS': str(artifacts), 'OPENROUTER_API_KEY': secret}
        driver = subprocess.run(['node', str(ROOT / 'scripts/browser_driver.cjs')],
                                input='{"op":"launch"}\n{"op":"goto","url":"https://example.test/?key=%s"}\n' % secret,
                                cwd=ROOT, env=environment, capture_output=True, text=True, timeout=60)
        self.assertEqual(driver.returncode, 0, driver.stderr)
        response = json.loads(driver.stdout.strip().splitlines()[-1])
        self.assertFalse(response['ok'])
        self.assertNotIn(secret, driver.stdout)
        files = sorted(path.name for path in artifacts.iterdir())
        self.assertEqual(len(files), 2, files)
        report = json.loads(next(path for path in artifacts.iterdir() if path.suffix == '.json').read_text())
        self.assertNotIn(secret, json.dumps(report))
        self.assertIn('fake navigation failed', report['error'])
        self.assertEqual(report['errors'], ['fake console error'])
        self.assertEqual(report['pages'], ['https://example.test/?key=%s' % secret.replace(secret, '<redacted>')])
        self.assertIn('link_html', report)
        self.assertTrue(str(report['screenshot']).startswith(str(artifacts)))
        self.assertTrue(Path(report['screenshot']).is_file())
