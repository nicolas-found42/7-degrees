# Verification and evidence

Run `python3 scripts/check.py --profile full` for a local delivery check. The runner prints each check and writes a fresh directory under ignored `target/verification/`. Every check runs: a failing check is recorded with its exit code and the run continues, so one failure never hides the state of the others. Existing output directories are refused.

| Profile | Scope | Prerequisites |
|---|---|---|
| `portable` (default) | Rust workspace tests, Python tests, formatting, host Clippy, committed snapshot pins, immutable historical audit, frozen semantic audit, published-report index, runtime diagnostics capture | Python 3.12+, Rust with rustfmt/Clippy, full Git history |
| `browser` | WASM build/Clippy and every explicitly ignored browser/canvas journey | wasm32 target, matching wasm-bindgen CLI, installed Node/Playwright/Chromium |
| `full` | Portable and browser profiles | Both sets above |
| `raw` | Pinned archives, cached pages and committed derived hashes | Existing ignored raw data; `--data-dir` selects it |

GitHub Actions runs portable and browser jobs separately on pushes and PRs. It installs browser dependencies and obtains the wasm-bindgen CLI version from Cargo.lock. Both jobs upload receipts even when a check fails. CI does not download the raw datasets or invoke paid semantic providers.

The browser matrix runs one journey at a time (`--test-threads=1`). Simultaneous Chromium journeys produced intermittent search, page-load and native evidence-tab timeouts on the local macOS host; the same assertions passed when serialized. This reduces concurrent browser resource use and native-input contention, at the cost of parallel test speed. It does not establish the precise cause of those timeouts. Each test app still runs on a multithreaded server, and portable Rust tests retain their normal parallel execution. See the [adversarial teardown receipts](../reports/adversarial-teardown-2026-10-12/README.md).

## Receipts and review

Each run writes `manifest.json` with the source commit, working-tree status and tracked-diff hash and untracked-file hashes, expected check list, actual argument vectors, start/end times, exit codes, log hashes and parsed test results. It also records the environment it ran in (toolchain and platform facts, and the names — never the values — of the environment variables that were present), the CI run id and URL when the run happens in GitHub Actions, and every artifact a check produced with its hash. `review-summary.json` omits the full test-name lists and retains counts and log pointers. Supply that compact receipt once as model-review evidence (the `evidence` of `jev_gate`); attach relevant raw excerpts for behavioral claims. Jev review runs from the agent session only; `scripts/check.py` and CI stay free of paid semantic providers. Complete logs remain authoritative and accessible. Review the recorded working-tree state when relating a run to a later commit.

Publish a successful run with:

```sh
python3 scripts/publish-final-evidence.py --raw-root target/verification/<run> --output docs/reports/<new-report>/checks
```

The publisher validates the complete manifest, every exit code, log hash and parsed summary before writing output. It rejects missing, failed, changed or incomplete receipts and existing evidence directories. Its published manifest preserves raw hashes and adds sanitized-log hashes. It cannot import older unrecorded logs as successful receipts. Earlier publications retain their original format and remain historical evidence.

A failed run is published the same way, under `failures/`, and only there:

```sh
python3 scripts/publish-final-evidence.py --raw-root target/verification/<run> --output docs/reports/failures/<yyyy-mm-dd>-<slug>/checks
```

The publisher refuses a failed run outside a `failures/` directory and refuses a successful run inside one, so a published run always says which kind it is. The slug names the failure; the directory keeps the receipts, the artifacts every check produced and the failed check's log — which is what a later diagnosis needs.

The full browser profile exercises this graph state matrix through existing tests:

| State or transition | Check |
|---|---|
| Empty snapshot | `api_integration`: HTTP 200 zero-player empty state (portable Rust suite) |
| Whole network with every player and highlighted-free payload | `browser_e2e`: compares payload counts and asserts no selected path |
| Player selection and centering | Same journeys select by pointer, keyboard and list |
| Relationship selection and evidence panel | Same journeys load the fixture and real Acy/Bogut evidence frames |
| Whole network over the real 5,106-player graph | `browser_e2e` and `canvas_browser` |
| Bounded graph pointer and keyboard controls | Existing canvas browser journey |

Browser assertions read counts and state from DOM attributes or the embedded payload. Response bodies are collected only when a test launches with `captureResponses: true`; this is required for credential-canary assertions. Incomplete capture fails those assertions closed. Application console/page errors are recorded separately.

## Runtime diagnostics

The portable profile ends with a `runtime-diagnostics` check: it builds the `app-server` binary, starts it against the committed reports with `NBA_RUNTIME_LOG_PATH` and `NBA_BUILD_COMMIT` set, provokes a semantic judgment, reads the served surfaces and keeps what the run produced. The check fails loudly when the server does not answer `/api/diagnostics`, when the diagnostics body breaks its contract (a commit that is not a 40-hex commit, a missing request count, a request path that carries a query string, an unknown semantic failure code, an event that carries a provider receipt) or when the server writes no runtime request log.

Its artifacts land in the check's artifact directory and are published with the rest of the run:

| Artifact | Content |
|---|---|
| `runtime-diagnostics.json` | The collected bodies, their elapsed times and the diagnostics contract as served |
| `runtime-request-log.jsonl` | The server's own JSONL runtime log: one JSON object per line, tagged with `kind` |
| `server.log` | The server's stdout and stderr for the run |

The server surfaces behind that check are described in the [README](../../README.md): `/api/diagnostics` (build, snapshot digests, served requests, semantic events, client reports), `/api/diagnostics/client` (the client failure channel) and `NBA_RUNTIME_LOG_PATH` (the operator's log file).

## Maintained PR description

Preserve large generated appendices without loading them into agent context:

```sh
gh pr view 19 --json body > target/pr-original.json
python3 scripts/pr-body.py --input target/pr-original.json
python3 scripts/pr-body.py --input target/pr-original.json --replacement target/pr-description.md --output target/pr-composed.md
gh pr edit 19 --body-file target/pr-composed.md
```

Before `gh pr edit`, `jev_verify` the replacement body's claims against the run's receipts.

The helper prints only the maintained prefix and preserves the recognized Qodo suffix byte for byte. An unrecognized Qodo boundary fails closed for inspection. Use the relevant PR number; the example refers to the existing explorer PR.
