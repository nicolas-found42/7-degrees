## Problem Statement

Basketball fans cannot easily answer how any two NBA-era players are connected through actual teammate relationships, or explore the shortest chain between them across league history. Same-franchise history is not enough: two players who were on a franchise at different times must not be linked. The result also depends on historical player and roster records whose coverage and precision vary by era.

## Solution

Build a local-first, Rust application that represents each NBA/BAA player as a node and creates an undirected teammate edge only when their roster tenures with the same team overlap in time. Let users search for players, inspect direct connections and shortest chains, explore nearby players, and see network-wide separation statistics. Use deterministic code for identity-backed graph construction, shortest paths, and counts. Use TypeSafe/Jev for bounded semantic judgments—query interpretation, candidate selection, clarification, and evidence checks—without letting model judgments invent graph facts.

## Current Workflow and Impact

This is a greenfield feature request: the repository has no application, imported player data, or existing graph workflow. There is no current UI state or failure to reproduce, and no occurrence rate or existing workaround was measured. The user wants to answer player-to-player connection questions and explore the chain from a local application rather than assemble historical rosters and paths manually.

## User Stories

1. As a basketball fan, I want every player with an official NBA or BAA game appearance represented as a node, so that the graph spans league history.
2. As a basketball fan, I want the player’s displayed identity to be stable across spelling, name, and source variations, so that the same person is not split into duplicate nodes.
3. As a basketball fan, I want to search by player name, so that I can start exploring from a known player.
4. As a basketball fan, I want common nicknames and reasonable spelling variations to find likely players, so that I do not need to know a source-specific spelling.
5. As a basketball fan, I want ambiguous names to show candidate players and useful era/team context, so that I can select the intended person.
6. As a basketball fan, I want the app to say when no candidate appears to match, so that it does not silently substitute a different player.
7. As a basketball fan, I want to ask in ordinary language for a connection between two players, so that I can use the graph without knowing its internal search controls.
8. As a basketball fan, I want to ask for a player’s teammates, nearby network, or profile, so that I can explore without specifying a second player.
9. As a basketball fan, I want to constrain a query by a stated era or other supported filter, so that I can focus an exploration without changing the underlying relationship rules.
10. As a basketball fan, I want the app to ask a concise follow-up when a player mention or query is ambiguous, so that uncertain interpretation does not produce a misleading path.
11. As a basketball fan, I want a direct teammate relationship to require overlapping roster tenure with the same team, so that franchise history alone does not create a false edge.
12. As a basketball fan, I want a mid-season move to connect a player only to teammates whose roster tenure overlapped with that player, so that pre- and post-move teams are handled correctly.
13. As a basketball fan, I want injury or non-appearance not to erase an otherwise evidenced roster overlap, so that teammate status is based on roster tenure rather than shared game minutes.
14. As a basketball fan, I want non-overlapping stints with the same franchise to remain unconnected, so that a historical succession is not mistaken for a teammate relationship.
15. As a basketball fan, I want All-Star, national-team, summer-league, and G League associations excluded from NBA/BAA teammate edges, so that the graph has one consistent league scope.
16. As a basketball fan, I want a player pair to have at most one graph edge even if they overlapped for multiple seasons or teams, so that path length measures relationship steps rather than repeat seasons.
17. As a basketball fan, I want the smallest number of teammate links between two players, so that their degree of separation is unambiguous.
18. As a basketball fan, I want to see a shortest chain as ordered player nodes and teammate links, so that I can understand how the connection works.
19. As a basketball fan, I want multiple equally short chains to remain available, so that I can compare alternative connections rather than seeing an arbitrary one only.
20. As a basketball fan, I want equally short chains optionally ranked by stated interests such as era span or surprising player links, so that the most engaging path can be surfaced without changing its degree.
21. As a basketball fan, I want the graph to distinguish a direct teammate link from an indirect chain, so that the degree count is easy to understand.
22. As a basketball fan, I want to expand a player’s direct and nearby connections in an interactive graph, so that I can explore beyond a single answer.
23. As a basketball fan, I want to pan, zoom, and focus the graph on a selected chain or neighborhood, so that a dense historical network remains navigable.
24. As a basketball fan, I want a player’s team/era context and edge provenance available from the graph, so that I can understand and inspect why a link exists.
25. As a basketball fan, I want a no-path result to distinguish known disconnected players from unresolved source coverage, so that missing data is not presented as proof of no historical connection.
26. As a basketball fan, I want to see network-wide separation statistics, including the separation histogram, connected components, and graph diameter, so that I can understand the shape of the NBA teammate network.
27. As a basketball fan, I want graph statistics to define how disconnected components and unreachable pairs are handled, so that the reported diameter and distribution are interpretable.
28. As a data maintainer, I want bulk historical data to be preferred over thousands of individual API calls, so that the initial build avoids unnecessary rate limits and repeated scraping.
29. As a data maintainer, I want independent historical datasets to cross-check player identity, player-season-team membership, team aliases, and selected dated roster events, so that one source’s omissions are detectable.
30. As a data maintainer, I want each imported source and derived edge to retain enough provenance to trace its evidence, so that questionable links can be investigated.
31. As a data maintainer, I want uncovered eras and conflicting roster records made visible in validation output, so that incomplete evidence is not silently converted into graph facts.
32. As a data maintainer, I want a reproducible, pinned data snapshot and an update process for the current season, so that graph changes can be compared and audited.
33. As a local user, I want the application to run on my machine without a separate database service, so that the graph is easy to start and explore.
34. As a local user, I want TypeSafe credentials to remain on the server side and out of browser assets and logs, so that semantic features do not expose credentials.
35. As a local user, I want the app to remain useful when Jev is unavailable, so that deterministic player lookup, graph traversal, and browsing still work.
36. As a local user, I want graph facts and shortest-path lengths computed deterministically, so that model uncertainty cannot change factual relationship results.
37. As a fan using assistive technology, I want player names, path degree, and selected graph relationships available in accessible text controls, so that canvas visualization is not the only way to use the app.
38. As a fan, I want the app to explain when an answer is a semantic interpretation rather than a verified graph fact, so that I can calibrate my trust in it.

## Concrete Example

Use synthetic fixture records, not claims about real players: Player A is on Team Red during [day 1, day 5), Player B is on Team Red during [day 2, day 4), Player B is on Team Blue during [day 10, day 13), and Player C is on Team Blue during [day 11, day 12). A–B and B–C are teammate edges, so A and C are two degrees apart unless an independently evidenced direct edge also exists. If Player D joins Team Red only after A's tenure ends, their shared franchise history alone creates no edge. This fixture illustrates expected behavior and has not been run against an implementation.

## Alternatives Considered

- **Season roster overlap versus actual tenure overlap:** The user selected actual time-overlapping roster tenure; season membership alone is insufficient for an edge.
- **Shared game appearance versus roster tenure:** Use roster tenure rather than shared minutes, so an injured or inactive player counts when the overlapping roster evidence exists.
- **Per-season API calls or a full historical page scrape versus bulk data:** Prefer one bulk dataset for the baseline and cross-check it with independent sources; use only a cached, rate-limited subset of dated web records where required. A full scrape is not the default because it has thousands of requests and observed throttling.
- **Full-network canvas versus focused exploration:** Start with the requested player, shortest chain, and a local neighborhood; make broader graph exploration available without attempting to render every dense edge at once.
- **JavaScript graph stack versus Rust-only app code:** Keep application code in Rust as requested. Leptos/Axum plus Rust canvas bindings is a research recommendation, not a confirmed crate-level decision. WASM bootstrap glue may still generate JavaScript.

## Evidence and Open Questions

- This is a proposed feature, not an observed product defect; no screenshot is applicable because no app UI exists yet.
- Candidate primary source: [Kaggle NBA Database](https://www.kaggle.com/datasets/wyattowalsh/basketball) (v238, CC BY-SA 4.0). Verified against the actual artifact: the archive (730,695,607 bytes compressed; 4,660,601,876 bytes uncompressed across 20 files) contains the 16 legacy tables only (player, team, game, play_by_play, etc.); the nbadb repo's published 261-table star schema (`bridge_player_team_season`, `fact_player_game_log`) is **not present in v238**, so player-season-team membership comes from the independent [NBA/ABA/BAA season statistics](https://www.kaggle.com/datasets/sumitrodatta/nba-aba-baa-stats) dataset instead (31,701 NBA/BAA player-season-team rows, 5,105 players, 1947–2026), with [player info up to 2025](https://www.kaggle.com/datasets/romainmorleghem/nba-players-info-and-headlinestats-up-to-2025) (5,135 players) as a third universe check. See `docs/data/source-manifest.md`.
- Candidate independent checks: [NBA/ABA/BAA season statistics](https://www.kaggle.com/datasets/sumitrodatta/nba-aba-baa-stats) and Basketball-Reference [transaction records](https://www.basketball-reference.com/leagues/NBA_2024_transactions.html). A prior source inventory's claim of transaction pages for 1951 onward and none for 1947–1950 was wrong: transaction pages were fetched HTTP 200 on 2026-10-09 for BAA_1947 (covering July 1, 1946 to June 30, 1947), BAA_1948, and NBA_1950, with day-precision dated rows — an earlier HTTP 429 was transient. The earliest BAA tenure reconstruction is a bounded parsing risk (fuzzy-dated rows), not a source absence.
- Basketball-Reference requests must be cached and throttled; research observed Cloudflare 1015 throttling during rapid fetches. The official stats site returned HTTP 403 from this environment. Do not treat any of these network responses as evidence that underlying historical records do not exist.
- Kaggle access is provisioned locally (CLI 2.2.4; `~/.kaggle/access_token`, mode 600) and was used to acquire and verify the pinned bulk snapshot on 2026-10-09; archive contents, actual data coverage, and license requirements are verified in `docs/data/source-manifest.md`.
- Runtime/model constraints are not yet tested in an application. No browser screenshots or app experiments exist. Keep these as implementation and evaluation work, not completed evidence.

## Implementation Decisions

- **Domain rules:** The graph is undirected. Nodes represent people with at least one official NBA/BAA regular-season or postseason game appearance; ABA-only players are outside the player universe. A teammate edge requires an evidenced, positive time overlap between roster tenures on the same team. Same-team tenure at different times is not an edge. Non-NBA/BAA team settings (All-Star, national team, summer league, G League) do not create edges. Injured or inactive players still count when their official roster tenures overlap. Multiple overlaps collapse to one edge.
- **Temporal evidence:** Model roster tenure as dated intervals reconstructed from transaction records and other source evidence. Do not use season-only membership as proof of simultaneous tenure. Where source records provide only a date and transaction ordering is unresolved, flag the case for review instead of inventing overlap. Preserve explicit data-coverage gaps; absence of a source record is not proof that a player or edge did not exist.
- **Historical source strategy:** Bulk Kaggle downloads (one request each, no per-page rate limits) are the baseline: the nbadb NBA Database (v238) for the player/team/game universe, the independent NBA/ABA/BAA statistics dataset for player-season-team membership and franchise aliases, and a player-info dataset as a third universe check; all are pinned in `docs/data/source-manifest.md` with licenses and checksums. A carefully throttled, cached subset of Basketball-Reference transaction pages (verified live from 1946-47) supplements dated events. Use the NBA API only where reachable and useful; avoid thousands of season-by-season calls. Kaggle access is provisioned locally (`~/.kaggle/access_token`, mode 600; CLI 2.2.4). Verified finding: the nbadb v238 artifact contains only the 16 legacy tables — the published 261-table star schema is absent from it. Early-season exact-date roster evidence comes from those transaction pages; fuzzy-dated rows make the 1946–1950 BAA tenure reconstruction a bounded parsing risk rather than a source absence.
- **Coverage and provenance:** Keep a source/version manifest, franchise-alias crosswalk, roster-event evidence, and derived graph provenance. Attribute and comply with source licenses; the candidate primary dataset is described as CC BY-SA 4.0 (verified). Report which seasons and records are directly evidenced, cross-checked, inferred, or unresolved. Do not claim complete date-level coverage until the evidence supports it.
- **Graph behavior:** Compute connected components, degrees, shortest paths, path lengths, and separation statistics in ordinary Rust code. Use BFS (or an equivalent exact unweighted shortest-path algorithm) for teammate distance. Keep semantic ranking separate: Jev may order paths of equal minimum length by explicit user preferences, but it must not choose a longer path over a shorter one or assert an edge that is absent from the graph.
- **Visualization and interaction:** Provide player search, player context, direct-neighbor exploration, shortest-chain display, and a navigable graph view. The recommended initial interaction is a focused chain/ego-network view rather than rendering the full dense network at once. Provide a text-accessible representation of selected nodes and paths. Show the separation histogram over reachable unordered player pairs, connected-component count, diameter, and unreachable-pair count; define diameter as the maximum finite shortest-path length within connected components, and report unreachable pairs separately.
- **Rust constraint:** Keep application-authored code in Rust, with no JavaScript graph or UI library. Research recommends Leptos with Axum for the local web application, a WASM canvas rendered via Rust browser bindings, and an in-memory graph representation using `petgraph`; this stack is a recommendation to validate during bootstrap, not a user-selected crate-level decision. Generated WASM JavaScript glue is unavoidable and does not constitute a hand-written JS application.
- **Local operation:** Serve the app on localhost and avoid a separate database server. The final storage encoding should be selected after inspecting the real dataset size and runtime access needs. Do not check credentials or source data into the repository.
- **Jev at runtime:** Use server-side TypeSafe/Jev for narrow semantic judgments where exact code or lexical rules are insufficient:
  - classify a free-text request into supported operations (for example, connect, player profile, neighbors, compare, or unsupported) and identify which bounded arguments/filters are stated;
  - retrieve a lexical candidate set in code, then use Choice/Score to resolve nicknames, aliases, misspellings, and ambiguous player mentions; pair candidate selection with a separate Noul existence check so a forced top choice cannot turn a no-match into a player;
  - confidence-gate low-certainty resolutions to user clarification; include explicit no-match/unsupported options;
  - rank only already-computed, equally short chains on separately defined semantic dimensions (such as era span or fit to the user's stated “interesting” criterion), and combine dimension scores with explicit code-owned weights;
  - choose among grounded, precomputed explanation formats or evidence snippets; graph facts and generated explanatory claims must be checked against the graph/source evidence before display.
- **Jev during development/data preparation:** Screen fetched or pasted external text before using it; use bounded extraction for source values when the source presents multiple candidates; audit extracted values against their original text; compare conflicting source passages; verify factual claims against supplied evidence; and classify transaction/record types in batches when labels are genuinely semantic. Keep deterministic identifiers, dates, transaction parsing, interval arithmetic, and validation rules in code. Treat every Jev output as fallible and retain its evidence and confidence/status.
- **Jev data minimization and security:** Send only the candidate names, relevant player context, requested operation, or source excerpt needed for a judgment—not the complete player graph. Keep API credentials server-side. The Rust server will call the documented TypeSafe interface using an HTTP client unless a supported Rust SDK is verified; the exact OpenRouter-compatible request configuration must be confirmed before implementation. Graph search must remain available if the judgment provider is unavailable.
- **Evaluation:** Compare Jev-assisted query/mention resolution against a lexical-only baseline on labeled cases, including nicknames, misspellings, ambiguous names, no-match cases, era clues, and hostile or off-topic input. Evaluate intent routing and alternative-chain ranking with labeled examples. Test prompt/criteria variants on the same cases; report accuracy, abstention/clarification rate, latency, tokens, and cost. Use thresholds calibrated on the evaluation set rather than assuming confidence guarantees correctness.
- **Source validation:** Compare player universe and season-team records across independent sources; stratify manual checks by BAA 1946–49, early NBA 1950–66, expansion 1967–80, merger/transition years, modern 1981–99, and 2000–2025/26. Review 50 sampled records per era where source data allows, all BAA/defunct-franchise cases, and all unusually complex multi-team seasons. Check known mid-season moves against dated source records. Record source disagreements instead of silently selecting one.
- **Testing seams:** Use two user-approved black-box seams: (1) Rust API integration tests against a small, deterministic historical roster fixture that exercise the data-to-graph path, identity, temporal edge rules, shortest paths, statistics, no-path behavior, and provider-failure fallback; (2) browser end-to-end tests against the running local app that verify rendered search, disambiguation, path display, graph navigation, filters, accessible text, and user-visible uncertainty. Assert externally observable results, not private implementation details. The repository contains no existing code or test suite, so there is no prior-art test pattern to preserve.
- **Acceptance evidence:** Include reproducible build/test output, data source/version manifest, coverage and gap report, labeled Jev experiment results with token/cost/latency measurements, and a final graph-statistics report. Run the real checks before completion and review the final changes against evidence.

## Acceptance Criteria

1. [ ] A pinned build dataset creates one player node for each evidenced NBA/BAA regular-season or postseason appearance in scope and excludes ABA-only players.
2. [ ] Names and team aliases from the chosen sources resolve to stable person and team identities; two different people with the same display name remain distinct.
3. [ ] Entering an exact player name through the application returns that player with era/team context.
4. [ ] Nickname, alias, and misspelling queries use a retrieved candidate set; evaluation reports labeled accuracy and false-match/abstention rates against a lexical-only baseline.
5. [ ] When multiple candidates remain plausible, the application shows disambiguating context and lets the user choose; it does not silently pick one.
6. [ ] When no candidate matches, the application presents a no-match result instead of forcing the highest-ranked candidate.
7. [ ] A natural-language connection request routes to the graph connection operation; player profile, neighbor, comparison, and unsupported requests reach their own visible outcomes.
8. [ ] A supported era/team filter changes the returned view or query as stated while leaving the underlying teammate graph definition unchanged.
9. [ ] When a required player mention or request argument remains uncertain, the application asks for clarification before showing a path.
10. [ ] Two players with an evidenced positive overlap in roster tenure on the same team have one undirected teammate edge, even if they never appeared in the same game.
11. [ ] Two players on the same franchise only in non-overlapping tenures have no direct edge, including across team-name or city changes.
12. [ ] A player who changes teams mid-season is linked only to players whose same-team roster tenure overlaps the relevant stint; season-level co-membership alone does not create the edge.
13. [ ] An injured or inactive player with an evidenced overlapping roster tenure remains a teammate; shared game minutes are not required.
14. [ ] All-Star, national-team, summer-league, and G League associations do not create NBA/BAA teammate edges.
15. [ ] Multiple overlapping seasons or repeated source rows produce exactly one graph edge for a player pair.
16. [ ] For an evidenced direct-teammate pair, the displayed degree is 1; for a pair whose shortest chain uses one mutual teammate and has no direct edge, it is 2.
17. [ ] A connection result displays the shortest chain as ordered players and links, and its edge count equals the returned degree.
18. [ ] If multiple shortest chains exist, the user can inspect those alternatives rather than receiving only an arbitrary non-explained choice.
19. [ ] Jev may rank only equal-length shortest chains using defined criteria; it never changes the exact minimum degree or adds an unsupported edge.
20. [ ] The graph view can focus on the selected chain and expand a selected player's direct/nearby connections without requiring the entire graph to be legible at once.
21. [ ] The user can pan, zoom, and refocus the displayed graph; a selected path and its degree remain identifiable during exploration.
22. [ ] Selected players and paths have an accessible text/list representation containing player names, links, and degree, in addition to the canvas view.
23. [ ] Opening a teammate link shows the team and date/season evidence used to establish its overlap, or explicitly states that the source evidence is incomplete.
24. [ ] A no-path response distinguishes a verified disconnected component from a result affected by unresolved source coverage.
25. [ ] The statistics view reports the separation histogram over reachable unordered pairs, connected-component count, graph diameter within components, and unreachable-pair count.
26. [ ] The statistics computation is deterministic and defines how isolates, unreachable pairs, and multiple components affect each reported value.
27. [ ] Data preparation downloads bulk source artifacts where available and records the source/version and request count for supplemental retrieval; it does not default to thousands of unthrottled individual requests.
28. [ ] Player universe, player-season-team membership, team aliases, and selected date-level events are cross-checked against independent sources; disagreements are listed for review.
29. [ ] Historical validation includes stratified records across the specified eras, BAA and defunct franchises, complex multi-team seasons, and known mid-season moves; the sample and outcomes are retained.
30. [ ] Every imported artifact and derived edge can be traced to its source version and supporting records; required source attribution/license terms are visible in project documentation.
31. [ ] Uncovered or ambiguous intervals are surfaced in a coverage report and are not silently treated as proven non-teammate relationships.
32. [ ] The app's coverage description does not claim complete exact-date history until the source audit demonstrates that coverage; unresolved 1946–1950 BAA tenure evidence is called out until resolved.
33. [ ] A documented local command starts the application without requiring a separately managed database service.
34. [ ] API credentials are server-side only and do not appear in browser assets, browser-visible responses, or routine logs.
35. [ ] If Jev is unavailable or unconfigured, deterministic player browsing and graph distance/path features continue to work and the UI reports semantic features as unavailable.
36. [ ] Graph identity, edge construction, shortest paths, degrees, components, and statistics are computed by ordinary deterministic Rust code; Jev outputs cannot override them.
37. [ ] The UI distinguishes verified graph/source facts from semantic suggestions or rankings and does not present unsupported generated biography or chain claims as fact.
38. [ ] Both approved test seams pass: Rust API integration tests over the fixture-backed data-to-graph path and browser E2E tests through the running local application. Jev evaluations also report labeled accuracy, no-match/clarification behavior, wording/order sensitivity, token use, latency, and cost.

## Testing Decisions

- **Good tests:** Exercise observable behavior through the two approved seams. Use small, hand-checked, deterministic roster fixtures for temporal edge cases and a separate browser run for what a user sees. Avoid assertions about private structs, layout algorithms, or internal prompts unless they affect a user-visible contract. Treat source coverage and Jev accuracy as measured evaluation results, not facts inferred from implementation.
- **Rust API integration seam:** Feed fixture player identities, team aliases, roster-tenure events, and source-gap markers through the same supported data-build and app interface. Assert: overlapping tenures create an edge; non-overlapping stints do not; same-day ambiguous events are surfaced; repeated seasons do not duplicate an edge; shortest degree is minimal; all equal-length paths can be retrieved; disconnected players are distinguished from unresolved evidence; statistics handle components correctly; missing Jev configuration does not break deterministic graph behavior.
- **Browser end-to-end seam:** Start the local app against a fixture-backed graph. Verify player lookup and no-match behavior, ambiguous-name clarification, a direct and an indirect connection, display of degree and path, neighborhood navigation, graph/text accessibility, filters, and source/coverage warnings.
- **Data validation and experiments:** Cross-check independent datasets; sample all historical eras; inspect high-risk early-era/defunct-team records and multi-team seasons; verify dated mid-season movement cases. Maintain labeled sets for query intent, player mention resolution, no-match, and path-interest ranking. Measure lexical-only and Jev-assisted accuracy, false-match/abstention rates, order/wording sensitivity, latency, tokens, and cost. Keep arithmetic and exact date handling in executable tests.
- **Prior art:** None. The repository contains only initial project documentation and has no existing modules, API, UI, or tests.

## Out of Scope

- ABA-only players and edges formed only through ABA service.
- Edges based only on same franchise, same season, shared game minutes, All-Star teams, national teams, summer league, or G League assignments.
- Letting Jev infer player identities outside a retrieved candidate set, calculate degree/path length, create graph edges, or override deterministic graph results.
- Unverified free-form AI biographies or chain narratives presented as sourced fact. Initial explanations should be grounded in graph/source facts.
- Public multi-tenant deployment, accounts, collaboration, a remote database service, or a native desktop application.
- A JavaScript-based graph renderer or JavaScript UI framework.
- Non-teammate relationship graphs, such as opponents, coaches, family, or draft classmates.
- A claim of complete exact-date coverage for historical eras until the source audit establishes it.

## Further Notes

- The user chose exact time-overlap semantics for teammate links, a Rust-only application stack, the recommended broad verification/evaluation plan with graph diameter as the headline network statistic, a private GitHub repository, and separate API-integration and browser-E2E testing seams.
- Kaggle access was provisioned locally on 2026-10-09 (`~/.kaggle/access_token`, mode 600; CLI 2.2.4) and the pinned bulk snapshot was downloaded and verified against the actual archive; see `docs/data/source-manifest.md`.
- Research found a meaningful early-history risk: bulk season-team data is not a substitute for date-overlapping roster tenure. Dated transaction coverage for the earliest BAA seasons was initially undercounted — Basketball-Reference transaction pages are verified live from 1946-47, with some rows fuzzy-dated. A full-history “every player” claim must remain qualified until tenure reconstruction (T4) is validated.
- TypeSafe documentation describes Choice, Noul, and Score over shared structured state, with independent questions batched together. Choice always selects among supplied options, so no-match must be represented separately; candidate retrieval and graph calculations remain code-owned. No Rust SDK was verified during research.
