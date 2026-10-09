# Final review corrections — spec #1

The final Standards/Spec review found an unsupported continuity assumption between repeated signings, missing query candidate choices, and two duplicated source/fixture mappings. All four are corrected together.

## Repeated contract source regression

Pinned parsed transaction rows `bbr-transactions-parsed.csv:13880`, `:13962`, and `:13987` establish Reggie Williams (`willire02`) signing on March 6, 2014, signing again on March 28, and expiration April 7. Both signings are ten-day contracts; no intervening dated departure proves uninterrupted service. The former tenure at Git `776d48e`, line 28588, incorrectly certified March 6–April 7 and admitted an overlap with Mustafa Shakur’s March 16–26 tenure.

The generic walk now splits every fresh arrival when a prior spell is still open, with no guessed expiration date. The prior reporting bracket’s end is unanchored and its `repeat-signing-prior-end-unknown` note blocks graph eligibility. Duplicate same-day source arrivals are still deduplicated. A later independently anchored pair remains usable: current Williams rows 29331/29332 are uncertain March 6–28 and evidenced March 28–April 7; Shakur row 24295 remains evidenced March 16–26. Current `/api/connection?from=willire02&to=shakumu01` is **disconnected with unresolved coverage**; this corrects the invented edge without claiming they could never have been historical teammates. The actual response and Williams uncertainty are in [runtime APIs](runtime-apis.json).

The importer also blocks the legacy `repeat-signing-continues-open-stint` note, even if an old report labels it directly evidenced. Public HTTP regression verifies this runtime defense. Python regression verifies the split and retained later pair, plus same-day duplicate handling.

The rebuilt T4 SHA-256 is `bfe9f5f2b60615af1160a157f7c482ee7ea3b9c86390e28f87a46c863ae39c04`; raw archives and all 80 cached source pages are unchanged. Graph counts, the immutable audit’s current comparison, browser slice hashes/line mappings and all affected reports were regenerated from actual output. Original historical audit rows and reading outcomes remain unchanged.

## Query clarification

Candidate era/team context and explicit selection now appear in the query page. The actual browser submits “connect Dee Brown to Quincy Acy”, sees both identities, and executes no graph operation before choosing. Selecting the older Dee retains the original request, Connect operation and Acy endpoint. HTTP regression independently verifies RED/2000s filter retention and rejects choosing Acy as a Dee candidate. Continuation revalidates all IDs against code-retrieved shortlists and executes deterministically without another provider call. [Actual captures and browser assertions](../t16-browser-e2e.md#final-review-clarification-regression) show both states.

## Standards and frozen evidence

T3 and T4 now import the same lightweight franchise-era mapping helper. App fixtures reuse `fixture::fixture_teams()`.

The original paid semantic experiment remains immutable. Offline replay hash-verifies the fixed historical Git snapshot and reproduces its former source import policy only inside the evaluation binary; normal runtime cannot select that policy. All 132 measured outcomes still reproduce. Those historical measurements retain the superseded continuity assumption and do not certify current graph edges. [Evaluation provenance](../../evaluation/README.md#historical-snapshot-replay-after-source-correction) states the qualification.
