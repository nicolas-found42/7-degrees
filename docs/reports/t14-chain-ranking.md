# T14 equally short chain ranking — issue #15

`ranking::rank` computes a bounded shortest-chain page from the graph, scores only those alternatives with server-side Jev, and orders them by explicit Rust weights. `/api/rank` returns the result; `/rank` renders selection/paging and labels preference suggestions separately from the selected graph/source facts. The chain page links to ranking while preserving endpoints and cursor.

## Acceptance evidence

| Exact issue criterion | Observable implementation evidence |
| --- | --- |
| “Equal-length shortest chains can be ranked by the user's stated interest using defined semantic dimensions combined with code-owned weights.” | Each alternative receives separate era-span and interest-fit Scores. Defaults combine normalized scores with era weight 0.25 and interest weight 0.75. A worked two-chain HTTP fixture changes from deterministic Beta-first to Gamma-first for the stated Gamma interest; another actual local HTTP test changes the ordering when code-owned weights change. |
| “The exact minimum degree never changes; Jev never adds an unsupported edge or overrides a graph fact.” | The public service computes its own page, revalidates degree against BFS, endpoints, IDs, link count and each positive evidenced teammate link before scoring. The HTTP test compares every ranked chain's IDs, degree and links with the deterministic `/api/paths` result. The longer three-link route remains excluded. Providers return Scores only, never paths. |
| “The UI distinguishes verified graph/source facts from semantic rankings.” | Rendered HTML labels “Semantic suggestions for this page” separately from “Graph/source facts” and shows the exact selected degree/link payload. It states the order is a preference, not a fact, and that the scope is the current page rather than a global ranking. Failed/uncertain semantic calls visibly retain deterministic alternatives. |

## Runtime and evaluation hook

```rust
pub fn rank(
    graph: &TeammateGraph,
    catalog: &search::PlayerCatalog,
    jev: &JevHandle,
    input: &RankingInput,
    config: &RankingConfig,
) -> Result<RankingCall, RankingError>;
```

`RankingInput` contains canonical `from`/`to`, `interest`, resumable `cursor`, and `limit`. Interest is bounded to 160 characters; page size is 1–20. The code computes exactly the requested deterministic page and never enumerates all alternatives for ranking. `RankingResult` retains exact minimum degree, exact decimal total, cursor/next cursor, and `scope=current_page`. Each chain has a stable original page index, copied canonical player/link facts, raw dimensional scores/distributions/confidence and optional weighted score. Selecting an original index preserves that chain even if a later provider call changes ordering. Paging retains the stated interest; each page is ranked independently.

`RankingCall.measurements` contains the measured Jev receipts per call for #16. Browser output serializes only `result`. Batches contain at most four chains with two independent Scores each (eight questions). A failure in any batch restores the entire deterministic page and clears applied scores. Low confidence retains raw scores and deterministic order with status `uncertain`; zero-weight dimensions do not block a sufficiently certain used dimension.

Era-span criteria describe one era, neighboring eras and distant generations using supplied career/team context. Interest-fit criteria describe unsupported, partial and direct support from supplied names, eras and teams. Prompts treat the interest as data and exclude invented biography, identities or relationships. `RankingConfig` exposes code-owned weights, bounded criteria descriptions, `minimum_confidence`, prompt wording variant and actual chain/question order for evaluation. Score means are normalized by their scale's maximum before weighted averaging. Stable ties retain original graph order. The default confidence floor 0.50 is provisional for #16 calibration.

`RankingOrder::Reverse` reverses candidate state and reassigns the sorted `chain_00...` question keys to the reversed chain sequence. The local HTTP test checks the actual outbound mapping and both Score dimensions. Score level descriptions stay ordered from low to high, as required by the [TypeSafe Score contract](https://docs.typesafe.ai/primitives/score); the [composite-scoring guidance](https://docs.typesafe.ai/patterns/composite-scoring) supports combining separate dimensions with explicit code weights. Runtime and evaluation use this single prompt builder.

## Validation and limitations

Seven offline tests cover weighted ordering with exact path/link equality, rendered selection/page scope, hostile input and malformed/unlisted/wrong-type/out-of-range replies, deterministic ties and low confidence, cursor/input boundaries, self-chain/no-path behavior, actual local HTTP score/order/receipt wiring, five alternatives across multiple batches, later failure restoring the whole page, and a zero-weight dimension. Invalid responses mark semantic status unavailable. Provider failure leaves the original alternatives intact. HTML text is escaped and canonical IDs/query values are URL-encoded; no authored JavaScript is added.

One opt-in real provider smoke used explicitly declared synthetic chains, not historical relationships. It returned interest-fit confidence 0.98/0.95 but era-span confidence 0.43/0.46. The actual result was `uncertain`, with deterministic order retained under the provisional 0.50 floor. The floor was not lowered for a passing outcome. The receipt records 214.4 ms, 1,309 input tokens, 74 output tokens and USD 0.000054978, model `typesafe/jev-1.13-20260917`. This demonstrates runtime transport and safe uncertainty behavior, not broad ranking accuracy; held-out evaluation and calibration remain #16 work.

External evidence directory: `/Users/Nicolas/Documents/github/7-degrees-review-notes/t14/`, containing red/green logs, full checks, actual live receipt and raw implementation diff. Independent browser interaction and final completion judgment are not claimed by these HTTP/HTML checks.
