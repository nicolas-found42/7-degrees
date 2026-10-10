# Why Maravich–Abdul-Jabbar is disconnected in the app

Investigated: 2026-10-10 against baseline commit `5da59649b9e3badf4baedeb03a4b9853c49b18ed`. Counts and tenure line references below describe that original snapshot. The [subsequent repair audit](link-repair/README.md) documents the corrected build. Scope: the running app, current source reports, and primary historical evidence. This investigation changes no implementation or imported data.

## Finding

The app's answer describes its restricted evidence graph. It does not establish historical impossibility. Neither endpoint has a certified tenure in the current export, so both are isolated before path search begins. A historical two-link route is supported by primary sources: **Pete Maravich → Gail Goodrich → Kareem Abdul-Jabbar**. This investigation establishes a route; it does not establish the globally shortest historical distance or enumerate all alternatives. Evidence and limitations follow below.

## Historical evidence

| Connection | Team and season | Evidence |
| --- | --- | --- |
| Maravich–Goodrich | New Orleans Jazz, 1976–77 | NBA explicitly identifies them as teammates. [NBA: Gail Goodrich profile](https://www.nba.com/news/history-nba-legend-gail-goodrich). |
| Goodrich–Abdul-Jabbar | Los Angeles Lakers, 1975–76 | Lakers' own statistical record lists Abdul-Jabbar in all 82 games and Goodrich in 75. They necessarily appeared in the same Lakers games. [Lakers 2023–24 media guide, printed p. 170, PDF p. 172](https://lalweb.blob.core.windows.net/public/lakers/media-relations/2023-24-Los-Angeles-Lakers-Media-Guide.pdf#page=172). |

The second edge is an inference from appearance counts, rather than an assumption that every pair of names on a season roster overlapped. The cited Lakers page supplies both the 82-game schedule and player appearance totals. It supports actual simultaneous teammate membership but does not, on its own, identify the dates of Goodrich's seven absences or certify uninterrupted tenure boundaries.

Don Chaney is another bridge worth checking with dated records. The same first-party Lakers guide lists Abdul-Jabbar in 82 games and Chaney in 81 during 1976–77, proving overlapping Lakers appearances. [Lakers media guide, printed p. 171, PDF p. 173](https://lalweb.blob.core.windows.net/public/lakers/media-relations/2023-24-Los-Angeles-Lakers-Media-Guide.pdf#page=173). Boston's own 2011–12 media guide lists Chaney's Celtics service as 1968–75 and 1977–80, and Maravich's as 1979–80. This is a primary team publication preserved on a third-party archive, rather than currently hosted by the team. [Celtics media guide, printed p. 149, PDF p. 152](https://library.sfo2.cdn.digitaloceanspaces.com/publications/basketball/yearbooks/KBOSCMG-2012.pdf#page=152). That Celtics season evidence alone is not a fully dated overlap proof, so the Goodrich route above is the stronger result established here.

## What the running app actually reports

On the investigation date, [`/api/connection?from=maravpe01&to=abdulka01`](http://127.0.0.1:3000/api/connection?from=maravpe01&to=abdulka01) returns `result: disconnected` and `certainty: unresolved_coverage`. The neighborhood endpoint for each player returns the endpoint alone with an empty links array. Those runtime observations explain why no search algorithm can currently recover the historical route: there is no first edge to traverse.

The import gate requires all four conditions: evidence class `directly-evidenced`, `start_anchored == 1`, `end_anchored == 1`, and no unresolved reasons. Other rows become coverage gaps instead of graph tenures. [Import code](../../crates/app-server/src/report_data.rs#L158).

The export contains the following counts. None qualifies for that gate. [Tenure export](t4/tenures.csv).

| Player | Total rows | Inferred | Cross-checked | Unresolved | Directly evidenced |
| --- | ---: | ---: | ---: | ---: | ---: |
| Pete Maravich | 11 | 6 | 2 | 3 | 0 |
| Kareem Abdul-Jabbar | 20 | 19 | 0 | 1 | 0 |
| Gail Goodrich | 14 | 10 | 2 | 2 | 0 |

Concrete examples from the same export:

- Maravich's 1980 Celtics row, line 16919, is `cross-checked`: `[1980-01-22, 1980-04-28)`, start anchored, end unanchored.
- Goodrich's 1976 Lakers and 1977 Jazz rows, lines 9769–9770, are unresolved, with no usable interval because a required season window is missing. His 1978–79 Jazz rows are inferred.
- Chaney's 1980 Celtics row, line 4793, is inferred with neither boundary anchored. His 1977 Lakers row, line 4789, anchors only arrival; his 1978 Lakers row, line 4791, anchors only departure.

Across the runtime export, only 2,227 of 30,057 tenures are certified (about 7.4%); 3,537 of 5,106 players have no certified tenure. This failure is part of a broad historical coverage limitation, not a peculiarity of these names. These counts come directly from the connection API's `coverage` object, which also reports 10,170 cross-checked, 13,445 inferred, and 4,215 unresolved exclusions.

## A concrete parser defect also loses historical evidence

The cached Basketball-Reference NBA 1975 transactions page contains the June 16, 1975 trade that sent Abdul-Jabbar from Milwaukee to Los Angeles. Its last transaction list item omits the explicit `</li>` before `</ul>`. [`TxnListParser.handle_endtag`](../../scripts/t4_fetch_bbr.py#L328) finishes a row only on `</li>`; closing the outer `</ul>` leaves the pending final row unfinished. The parsed export therefore misses this trade despite having its source HTML locally. [Parsed transactions](t4/bbr-transactions-parsed.csv); [first-party source page](https://www.basketball-reference.com/leagues/NBA_1975_transactions.html).

The NBA independently records Abdul-Jabbar's Milwaukee-to-Lakers trade on June 16, 1975. [NBA: This Date in the NBA, June](https://www.nba.com/news/history-this-date-in-nba-june).

Omitting the last `li` end tag when there is no further content in its parent is permitted HTML. The cached shape is supported HTML that this parser does not handle, rather than evidence that the transaction itself is malformed. [WHATWG HTML Standard: the `li` element](https://html.spec.whatwg.org/multipage/grouping-content.html#the-li-element).

A read-only diagnostic that inserts the omitted closing tag **in memory** produces 83 rows instead of 82 for NBA 1975 and recovers the trade. Across the 80 cached transaction pages it recovers 144 transaction paragraphs: 139 with precise dates and five with fuzzy dates. This measures the lost final-list-item evidence, not 144 distinct newly certifiable tenures or graph edges. Reproduce from the repository root:

```python
from pathlib import Path
import sys

sys.path.insert(0, "scripts")
from t4_fetch_bbr import parse_transactions_html

affected = recovered = 0
for path in sorted(Path("data/cache/bbr").glob("*_transactions.html")):
    html = path.read_text()
    diagnostic_html = html.replace("</p></ul>", "</p></li></ul>")
    league, year = path.name.split("_")[:2]
    original, _, _ = parse_transactions_html(html, league, int(year))
    diagnostic, _, _ = parse_transactions_html(diagnostic_html, league, int(year))
    seen = {(row["date_iso"], row["text"]) for row in original}
    added = [row for row in diagnostic
             if (row["date_iso"], row["text"]) not in seen]
    affected += bool(added)
    recovered += len(added)
    if path.name == "NBA_1975_transactions.html":
        print(path.name, len(original), len(diagnostic))
        print([row for row in added if "Kareem" in row["text"]])
print("affected pages:", affected, "recovered paragraphs:", recovered)
```

This replacement is a diagnostic, not a recommended production HTML parser fix. Production parsing should handle omitted list-item end tags, list closure, and the end of input correctly, with fixtures reproducing the actual markup.

## Repair direction and remaining limits

Repairing the parser should recover missing dated transactions, but that alone does not prove the full historical route under the current both-boundaries rule. The per-season reconstruction and strict import gate leave long-serving teammates without accepted tenures when their season records lack transaction-anchored starts or ends. The builder can carry some offseason arrivals into the next season, but it does not certify every intervening season of a multi-season career from widely separated arrival and departure events. [Reconstruction code](../../scripts/t4_reconcile.py#L579). To support historical connections faithfully, augment transaction evidence with dated roster-presence or game evidence, or reconstruct continuous tenure spans with explicitly recorded assumptions and uncertainty. Same-team same-season brackets should not automatically become certified edges: intra-season departures and arrivals can make those pairs non-overlapping.

Use the Maravich–Goodrich–Abdul-Jabbar route as a historical regression case. Verification must check that each edge has defensible simultaneous roster evidence and that the UI distinguishes source coverage from historical reachability. This report has not changed the graph, certified new exact day-counts, or tested a proposed implementation.
