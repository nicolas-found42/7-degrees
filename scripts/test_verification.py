import importlib.util
import json
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


class VerificationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)

    def run_case(self, exit_code=0):
        raw = self.base / 'raw'
        status = run_checks([('example', [sys.executable, '-c', f'print("test result: ok. 2 passed; 0 failed; 1 ignored;"); raise SystemExit({exit_code})'])], raw)
        return raw, status

    def test_failed_receipt_cannot_publish_and_stops_pipeline(self):
        raw = self.base / 'raw'
        self.assertEqual(run_checks([('failure', [sys.executable, '-c', 'raise SystemExit(3)']),
                                    ('never', [sys.executable, '-c', 'print("wrong")'])], raw), 3)
        self.assertFalse((raw / 'never.log').exists())
        with self.assertRaises(ValueError): publisher.publish(raw, self.base / 'published')
        self.assertFalse((self.base / 'published').exists())

    def test_completed_but_failed_receipt_is_rejected(self):
        raw, status = self.run_case(9)
        self.assertEqual(status, 9)
        self.assertTrue(json.loads((raw / 'manifest.json').read_text())['completed'])
        with self.assertRaises(ValueError): publisher.publish(raw, self.base / 'published')
        self.assertFalse((self.base / 'published').exists())

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
