# T6 player search — issue #8

The public `search::PlayerCatalog` owns deterministic lexical retrieval, reused by `/api/search` and the Rust-rendered `/search` view. Selecting a candidate opens `/players/{canonical_id}` with era/team context and a `/chain?from={canonical_id}` link. The home page has a labeled search form. Search never redirects to a guessed identity.

## Acceptance evidence

| Issue criterion | Behavior and retained verification |
| --- | --- |
| “Entering an exact player name returns that player with era/team context.” | Exact canonical names rank first with ID, first/last recorded seasons and canonical franchise list. The approved report-backed HTTP test verifies Michael Jordan, 1985–2003, BULLS/WIZARDS. A running server using the committed 5,106-player catalog returns Michael Jordan and LeBron James as `exact_match`. |
| “Nickname, alias, and misspelling queries return a ranked list from a code-built lexical shortlist.” | Source display-name aliases plus the provenance-bearing curated nickname CSV feed the same code shortlist. Tests cover Shaq, The Answer, Dr. J, Jose Calderon, Micheal Jordan, surnames and partial names. The running catalog returns the expected canonical players for those nicknames and the transposed spelling. |
| “The UI shows candidates with era/team context so the user can select the intended player.” | Rendered HTML tests verify distinct namesake IDs, different eras/teams, profile links, connection links and the retained coverage warning. Empty and no-match states are visible; unknown profile IDs render a 404. |

## Retrieval contract

`GET /api/search?q=<text>&limit=<1..50>` defaults to 20 candidates. Queries longer than 160 Unicode characters and limits outside 1–50 return HTTP 400. The reusable service enforces the same bounds. Results contain `query`, `status`, `candidates`, `matches_total` and `has_more`. Each candidate has canonical player context, `match_kind`, `matched_name` and a deterministic integer `score`; scores are ordering weights, not probabilities.

Normalization folds Unicode decomposition, combining marks, case, apostrophes and periods. Exact canonical names score 1000, exact aliases 950; token/prefix matches follow, then bounded Damerau edit distance including adjacent transpositions. Ties use canonical ID order. If an exact name or alias exists, retrieval keeps the matching identities rather than adding approximate names. It preserves separate canonical IDs for namesakes. Tied top candidates produce `ambiguous` before truncation, and the HTML view always asks the user to select a player. A normalized empty query returns `empty_query`; unrelated text returns `no_match`.

Era/team context comes from the canonical imported reports and includes players with incomplete dated roster coverage. It does not claim every listed membership is a certified teammate edge. Fixture-only catalogs explicitly show missing era context rather than deriving calendar years from synthetic dates. Profile and search text are HTML-escaped, IDs are URL-encoded, and the UI has no authored JavaScript.

## Alias provenance

`docs/data/player-search-aliases.csv` retains a source URL for each curated alias, keyed by canonical ID. Current aliases are Shaq ([NBA Archive 75](https://www.nba.com/news/archive-75-shaquille-oneal)), The Answer ([NBA Archive 75](https://www.nba.com/news/archive-75-allen-iverson)), Dr. J ([NBA legend profile](https://www.nba.com/news/history-nba-legend-julius-erving)), Air Jordan and MJ ([NBA legend profile](https://www.nba.com/news/history-nba-legend-michael-jordan)). The small catalog supplements imported source display-name aliases; it does not claim to cover every nickname.

## Checks

Seven new HTTP/HTML tests passed, developed in red/green slices for exact names, aliases, typo retrieval, ambiguity, rendered selection and bounds. `cargo test --workspace` passed 55 tests with one pre-existing opt-in live-provider test ignored. `cargo clippy --workspace --all-targets -- -D warnings` passed; formatting and whitespace checks passed. The typo and namesake tests exercise observable retrieval behavior through the approved report-backed API seam.

Retained external evidence directory: `/Users/Nicolas/Documents/github/7-degrees-review-notes/t6/`. It contains red/green logs, `workspace-tests.log`, `clippy.log`, real running-catalog JSON and rendered HTML. `real-catalog-search.json` records Johnson as 80 candidates, `ambiguous`, with the bounded first 20 shown, and unrelated text as `no_match`. Browser interaction has not been claimed here; the pipeline's independent UI review follows integration.
