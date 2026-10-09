# Glossary

- **NBA/BAA player**: A person with at least one official NBA or BAA regular-season or postseason game appearance. ABA-only players are excluded from the requested player universe.
- **Player node**: A unique NBA/BAA player in the graph, identified independently of name spelling or team aliases.
- **Roster tenure**: A time-bounded period in which a player was on an NBA/BAA team's roster, reconstructed from the best available dated source records.
- **Teammate edge**: An undirected connection between two player nodes whose roster tenures with the same team overlapped in time. Same-team membership in non-overlapping periods does not create an edge.
- **Degree of separation**: The number of teammate edges in a shortest chain between two player nodes. Direct teammates are one degree apart; a chain through one mutual teammate is two degrees apart unless the endpoints already have a direct edge.
- **Shortest teammate chain**: A minimum-edge path connecting two player nodes. More than one chain may have the same minimum length.
- **Franchise identity**: A canonical team identity used to reconcile names, locations, and abbreviations across sources. Franchise continuity alone does not create a teammate edge; the players' roster tenures must overlap.
- **Source coverage gap**: A period or record not established by the available source evidence. Missing evidence must not be treated as proof that a player, tenure, or edge did not exist.
