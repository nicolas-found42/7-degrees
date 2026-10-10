# Verification and evidence

Run `python3 scripts/check.py --profile full` for a local delivery check. The runner prints each check and writes a fresh directory under ignored `target/verification/`. It stops on the first failed command and records the failure. Existing output directories are refused.

| Profile | Scope | Prerequisites |
|---|---|---|
| `portable` (default) | Rust workspace tests, Python tests, formatting, host Clippy, committed snapshot pins, immutable historical audit, frozen semantic audit | Python 3.12+, Rust with rustfmt/Clippy, full Git history |
| `browser` | WASM build/Clippy and every explicitly ignored browser/canvas journey | wasm32 target, matching wasm-bindgen CLI, installed Node/Playwright/Chromium |
| `full` | Portable and browser profiles | Both sets above |
| `raw` | Pinned archives, cached pages and committed derived hashes | Existing ignored raw data; `--data-dir` selects it |

GitHub Actions runs portable and browser jobs separately on pushes and PRs. It installs browser dependencies and obtains the wasm-bindgen CLI version from Cargo.lock. Both jobs upload receipts even when a check fails. CI does not download the raw datasets or invoke paid semantic providers.

## Receipts and review

Each run writes `manifest.json` with the source commit, working-tree status and tracked-diff hash and untracked-file hashes, expected check list, actual argument vectors, start/end times, exit codes, log hashes and parsed test results. `review-summary.json` omits the full test-name lists and retains counts and log pointers. Supply that compact receipt once as model-review evidence; attach relevant raw excerpts for behavioral claims. Complete logs remain authoritative and accessible. Review the recorded working-tree state when relating a run to a later commit.

Publish a successful run with:

```sh
python3 scripts/publish-final-evidence.py --raw-root target/verification/<run> --output docs/reports/<new-report>/checks
```

The publisher validates the complete manifest, every exit code, log hash and parsed summary before writing output. It rejects missing, failed, changed or incomplete receipts and existing evidence directories. Its published manifest preserves raw hashes and adds sanitized-log hashes. It cannot import older unrecorded logs as successful receipts. Earlier publications retain their original format and remain historical evidence.

The full browser profile exercises this graph state matrix through existing tests:

| State or transition | Check |
|---|---|
| Empty snapshot | `stats_view`: empty network API and HTTP 200 full-field empty state (portable Rust suite) |
| Complete network and highlighted path | `browser_e2e`: full-network journey compares payload counts and highlighted path |
| Player search / zoom / fit | Same journey verifies the complete payload is retained |
| Relationship selection then isolated-player selection | Same journey verifies evidence URL is cleared and link hidden |
| Evidence navigation / desktop and phone chain layouts | Selected-chain browser journey |
| Bounded graph pointer and keyboard controls | Existing canvas browser journey |

Browser assertions read counts and state from DOM attributes or the embedded payload. Response bodies are collected only when a test launches with `captureResponses: true`; this is required for credential-canary assertions. Incomplete capture fails those assertions closed. Application console/page errors are recorded separately.

## Maintained PR description

Preserve large generated appendices without loading them into agent context:

```sh
gh pr view 19 --json body > target/pr-original.json
python3 scripts/pr-body.py --input target/pr-original.json
python3 scripts/pr-body.py --input target/pr-original.json --replacement target/pr-description.md --output target/pr-composed.md
gh pr edit 19 --body-file target/pr-composed.md
```

The helper prints only the maintained prefix and preserves the recognized Qodo suffix byte for byte. An unrecognized Qodo boundary fails closed for inspection. Use the relevant PR number; the example refers to the existing explorer PR.
