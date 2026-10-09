//! Deterministic teammate graph for fixture and canonical historical tenures.
//!
//! Domain rules (spec, Implementation Decisions):
//! - Nodes are player nodes, identified independently of display names.
//! - A teammate edge requires a positive time overlap of roster tenures on the
//!   same team. Same-franchise, non-overlapping tenures do not create an edge.
//! - A player's tenures are independent intervals; a stint only links them to
//!   players overlapping that stint.
//! - Repeated overlaps between the same pair collapse to one undirected
//!   teammate edge whose provenance accumulates all overlapping stints.

use num_bigint::BigUint;
use std::collections::{BTreeMap, HashMap};

use petgraph::graph::NodeIndex;
use petgraph::visit::{EdgeRef, VisitMap, Visitable};
use petgraph::{Directed, Graph};

/// A counted day in league time. Days are integers so interval arithmetic stays
/// exact; a real data import maps calendar dates onto this timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day(pub u32);

/// A time interval `[start, end)` — end is exclusive, matching the spec's
/// half-open tenure days (e.g. A on Team Red `[day 1, day 5)`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tenure {
    pub start: Day,
    pub end: Day,
}

impl Tenure {
    /// Positive overlap between two half-open intervals (empty otherwise).
    pub fn overlap_days(&self, other: &Tenure) -> u32 {
        let start = self.start.0.max(other.start.0);
        let end = self.end.0.min(other.end.0);
        end.saturating_sub(start)
    }
}

/// A named team in the fixture graph (a franchise identity).
#[derive(Clone, Debug)]
pub struct Team {
    pub id: String,
    pub name: String,
}

/// A player node: identified by a stable id, independent of display name.
#[derive(Clone, Debug)]
pub struct Player {
    pub id: String,
    /// Display name, as shown in the UI.
    pub name: String,
}

/// One evidenced roster stint of a player with a team.
#[derive(Clone, Debug)]
pub struct LocatedTenure {
    pub player: String,
    pub team: String,
    pub tenure: Tenure,
}

/// All roster tenures for all players; the input to graph construction. This is
/// the struct a real data import fills from bulk source records.
#[derive(Clone, Debug, Default)]
pub struct RosterData {
    pub players: Vec<Player>,
    pub teams: Vec<Team>,
    /// Each entry: one player's stint with a team.
    pub tenures: Vec<LocatedTenure>,
}

/// Provenance of one teammate edge: the team and the summed positive overlap
/// in days across all overlapping stints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeEvidence {
    pub team: String,
    pub overlap_days: u32,
}

/// An undirected teammate edge with its accumulated evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeammateEdge {
    /// Endpoint ids, in stable order (lexicographically smaller first).
    pub a: String,
    pub b: String,
    pub evidence: Vec<EdgeEvidence>,
}

impl TeammateEdge {
    /// Positive overlap in days, summed across all evidenced stints.
    pub fn overlap_days(&self) -> u32 {
        self.evidence.iter().map(|e| e.overlap_days).sum()
    }
}

fn ordered_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a <= b { (a, b) } else { (b, a) }
}

impl RosterData {
    /// Build the teammate edge list.
    ///
    /// Rule 1: for every shared team, a positive tenure overlap creates one
    /// teammate edge. Rule 2: repeated overlaps between the same pair collapse
    /// into that single undirected edge (evidence accumulates). Rule 3: stints
    /// that do not overlap create nothing, even within one franchise.
    pub fn teammate_edges(&self) -> Vec<TeammateEdge> {
        // Group tenures by team, then compare intervals within a team.
        let mut by_team: BTreeMap<&str, Vec<&LocatedTenure>> = BTreeMap::new();
        for tenure in &self.tenures {
            by_team
                .entry(tenure.team.as_str())
                .or_default()
                .push(tenure);
        }

        // (a, b) -> team -> summed overlap days for that team.
        let mut overlaps: BTreeMap<(&str, &str), BTreeMap<&str, u32>> = BTreeMap::new();
        for (team, mut team_tenures) in by_team {
            // Exact repeated source intervals retain their source pointers in
            // the import metadata, but count once in the graph.
            team_tenures.sort_by_key(|t| (t.tenure.start.0, t.tenure.end.0, &t.player));
            team_tenures.dedup_by(|a, b| a.player == b.player && a.tenure == b.tenure);
            for (i, a) in team_tenures.iter().enumerate() {
                for b in team_tenures.iter().skip(i + 1) {
                    if b.tenure.start >= a.tenure.end {
                        break;
                    }
                    if a.player == b.player {
                        continue;
                    }
                    let days = a.tenure.overlap_days(&b.tenure);
                    if days > 0 {
                        // Repeated overlaps from multiple stints on the same
                        // team accumulate their summed overlap days.
                        let (x, y) = ordered_pair(a.player.as_str(), b.player.as_str());
                        *overlaps.entry((x, y)).or_default().entry(team).or_insert(0) += days;
                    }
                }
            }
        }

        overlaps
            .into_iter()
            .map(|((a, b), team_overlaps)| TeammateEdge {
                a: a.to_string(),
                b: b.to_string(),
                evidence: team_overlaps
                    .into_iter()
                    .map(|(team, days)| EdgeEvidence {
                        team: team.to_string(),
                        overlap_days: days,
                    })
                    .collect(),
            })
            .collect()
    }
}

/// The built teammate graph: player nodes plus undirected teammate edges, with
/// the roster data retained as edge provenance.
pub struct TeammateGraph {
    pub roster: RosterData,
    graph: Graph<Player, Vec<EdgeEvidence>, Directed>,
    by_id: HashMap<String, NodeIndex>,
    neighbors: Vec<Vec<NodeIndex>>,
    statistics: std::sync::OnceLock<GraphStatistics>,
}

/// A chain between two players expressed in domain vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chain {
    /// Ordered player node ids from `from` to `to`.
    pub path: Vec<String>,
    /// Ordered teammate links; `links.len() == path.len() - 1`.
    pub links: Vec<LinkEvidence>,
}

/// One teammate link in a chain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkEvidence {
    pub from: String,
    pub to: String,
    /// The team that establishes the teammate relationship.
    pub team: String,
    /// The positive overlap in days evidenced for this link.
    pub overlap_days: u32,
}

/// A shortest chain plus its degree of separation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainWithDegree {
    pub path: Vec<String>,
    pub links: Vec<LinkEvidence>,
    /// Number of teammate links: `links.len()`, 0 for the trivial self-chain.
    pub degree: usize,
}

/// Outcome of a shortest-chain query between two known players.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    /// A shortest teammate chain exists.
    Connected(Chain),
    /// No teammate chain exists between the two players.
    Disconnected,
}

impl TeammateGraph {
    /// Build a graph from roster data using the domain rules above.
    pub fn build(roster: RosterData) -> Self {
        let mut graph = Graph::new();
        let mut by_id = HashMap::new();
        for player in &roster.players {
            let node = graph.add_node(player.clone());
            by_id.insert(player.id.clone(), node);
        }
        for edge in roster.teammate_edges() {
            let a = by_id[&edge.a];
            let b = by_id[&edge.b];
            graph.add_edge(a, b, edge.evidence);
        }
        let neighbors = graph
            .node_indices()
            .map(|node| {
                let mut neighbors: Vec<_> = graph.neighbors_undirected(node).collect();
                neighbors.sort_by(|a, b| graph[*a].id.cmp(&graph[*b].id));
                neighbors
            })
            .collect();
        TeammateGraph {
            neighbors,
            statistics: std::sync::OnceLock::new(),
            roster,
            graph,
            by_id,
        }
    }

    pub fn player_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.by_id.keys().cloned().collect();
        ids.sort();
        ids
    }

    pub fn contains_player(&self, id: &str) -> bool {
        self.by_id.contains_key(id)
    }

    /// All teammate edges, one per pair, in stable order.
    pub fn edges(&self) -> Vec<TeammateEdge> {
        let mut edges: Vec<TeammateEdge> = self
            .graph
            .edge_references()
            .map(|edge| TeammateEdge {
                a: self.graph[edge.source()].id.clone(),
                b: self.graph[edge.target()].id.clone(),
                evidence: edge.weight().clone(),
            })
            .collect();
        edges.sort_by(|x, y| (&x.a, &x.b).cmp(&(&y.a, &y.b)));
        edges
    }

    /// Shortest teammate chain from `from` to `to`, by BFS over teammate edges.
    ///
    /// The chain is minimal: BFS stops at the first layer that reaches the
    /// target, so its edge count is the degree of separation. When several
    /// equal-length chains exist this returns one of them; `all_shortest_chains`
    /// exposes every alternative at the same minimum length.
    pub fn shortest_chain(&self, from: &str, to: &str) -> Option<Connection> {
        let start = *self.by_id.get(from)?;
        let goal = *self.by_id.get(to)?;
        if start == goal {
            return Some(Connection::Connected(Chain {
                path: vec![from.to_string()],
                links: Vec::new(),
            }));
        }

        let mut predecessor: HashMap<NodeIndex, Option<NodeIndex>> = HashMap::new();
        let mut visited = Graph::<Player, Vec<EdgeEvidence>, Directed>::visit_map(&self.graph);
        let mut queue = std::collections::VecDeque::new();
        visited.visit(start);
        predecessor.insert(start, None);
        queue.push_back(start);

        while let Some(node) = queue.pop_front() {
            // The teammate graph is undirected; edges are stored once, so
            // traverse both directions.
            for &neighbor in &self.neighbors[node.index()] {
                if visited.is_visited(&neighbor) {
                    continue;
                }
                visited.visit(neighbor);
                predecessor.insert(neighbor, Some(node));
                if neighbor == goal {
                    // First arrival in BFS = minimal edge count.
                    let mut path = vec![goal];
                    let mut current = goal;
                    while let Some(Some(prev)) = predecessor.get(&current) {
                        path.push(*prev);
                        current = *prev;
                    }
                    path.reverse();
                    let links = self.chain_links(&path);
                    return Some(Connection::Connected(Chain {
                        path: path.iter().map(|n| self.graph[*n].id.clone()).collect(),
                        links,
                    }));
                }
                queue.push_back(neighbor);
            }
        }
        Some(Connection::Disconnected)
    }

    /// All shortest chains between two players (all minimal-edge paths).
    pub fn all_shortest_chains(&self, from: &str, to: &str) -> Option<Vec<ChainWithDegree>> {
        let start = *self.by_id.get(from)?;
        let goal = *self.by_id.get(to)?;
        if start == goal {
            return Some(vec![ChainWithDegree {
                path: vec![from.to_string()],
                links: Vec::new(),
                degree: 0,
            }]);
        }
        // Layered BFS recording ALL predecessors in the previous layer.
        let mut distance: HashMap<NodeIndex, usize> = HashMap::new();
        let mut predecessors: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        distance.insert(start, 0);
        queue.push_back(start);
        while let Some(node) = queue.pop_front() {
            let d = distance[&node];
            for &neighbor in &self.neighbors[node.index()] {
                match distance.get(&neighbor) {
                    None => {
                        distance.insert(neighbor, d + 1);
                        predecessors.insert(neighbor, vec![node]);
                        queue.push_back(neighbor);
                    }
                    Some(&existing) if existing == d + 1 => {
                        predecessors.entry(neighbor).or_default().push(node);
                    }
                    _ => {}
                }
            }
        }
        let goal_distance = *distance.get(&goal)?;
        // Collect all minimal paths by walking predecessors backwards.
        fn collect(
            distance: &HashMap<NodeIndex, usize>,
            predecessors: &HashMap<NodeIndex, Vec<NodeIndex>>,
            node: NodeIndex,
            suffix: &mut Vec<NodeIndex>,
            out: &mut Vec<Vec<NodeIndex>>,
        ) {
            suffix.push(node);
            if distance[&node] == 0 {
                let mut path = suffix.clone();
                path.reverse();
                out.push(path);
            } else {
                for prev in &predecessors[&node] {
                    collect(distance, predecessors, *prev, suffix, out);
                }
            }
            suffix.pop();
        }
        let mut node_paths = Vec::new();
        collect(
            &distance,
            &predecessors,
            goal,
            &mut Vec::new(),
            &mut node_paths,
        );
        let chains = node_paths
            .into_iter()
            .map(|path| {
                let links = self.chain_links(&path);
                ChainWithDegree {
                    path: path.iter().map(|n| self.graph[*n].id.clone()).collect(),
                    links,
                    degree: goal_distance,
                }
            })
            .collect();
        Some(chains)
    }

    /// Edge evidence for consecutive path nodes, following path order. For each
    /// link the strongest (largest-overlap) evidence entry is shown.
    fn chain_links(&self, path: &[NodeIndex]) -> Vec<LinkEvidence> {
        let mut links = Vec::new();
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let weight = self
                .graph
                .edges(a)
                .find(|edge| edge.target() == b)
                .or_else(|| self.graph.edges(b).find(|edge| edge.target() == a))
                .map(|edge| edge.weight().clone())
                .expect("consecutive chain nodes share a teammate edge");
            let evidence = weight
                .iter()
                .max_by_key(|e| e.overlap_days)
                .expect("edge has at least one evidence entry");
            links.push(LinkEvidence {
                from: self.graph[a].id.clone(),
                to: self.graph[b].id.clone(),
                team: evidence.team.clone(),
                overlap_days: evidence.overlap_days,
            });
        }
        links
    }

    /// A bounded page of ALL equally short alternatives in stable ID order.
    /// The predecessor DAG is counted without enumerating its paths. Offset
    /// skips entire subtrees, so a large offset does not allocate skipped paths.
    pub fn shortest_chains_page(
        &self,
        from: &str,
        to: &str,
        offset: u64,
        limit: usize,
    ) -> Option<ChainPage> {
        self.shortest_chains_at_rank(from, to, BigUint::from(offset), limit)
    }

    /// Resume the next equally short alternative using the cursor returned by
    /// a previous page. Cursor ranks and DAG counts have arbitrary precision.
    pub fn shortest_chains_page_cursor(
        &self,
        from: &str,
        to: &str,
        cursor: &str,
        limit: usize,
    ) -> Result<Option<ChainPage>, String> {
        let rank = cursor
            .strip_prefix("v1:")
            .ok_or("unsupported shortest-chain cursor")?;
        // A simple DAG has at most 2^n paths, so a valid rank has fewer
        // decimal digits than nodes. Reject oversized input before parsing.
        if rank.is_empty()
            || rank.len() > self.graph.node_count().max(1)
            || !rank.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("invalid shortest-chain cursor".into());
        }
        let rank =
            BigUint::parse_bytes(rank.as_bytes(), 10).ok_or("invalid shortest-chain cursor")?;
        Ok(self.shortest_chains_at_rank(from, to, rank, limit))
    }

    fn shortest_chains_at_rank(
        &self,
        from: &str,
        to: &str,
        offset: BigUint,
        limit: usize,
    ) -> Option<ChainPage> {
        let start = *self.by_id.get(from)?;
        let goal = *self.by_id.get(to)?;
        let distance = self.distances(goal);
        if distance[start.index()] == usize::MAX {
            return Some(ChainPage {
                chains: Vec::new(),
                total: 0,
                total_exact: "0".into(),
                total_saturated: false,
                next_offset: None,
                next_cursor: None,
            });
        }
        let mut order: Vec<_> = self
            .graph
            .node_indices()
            .filter(|n| distance[n.index()] != usize::MAX)
            .collect();
        order.sort_by_key(|n| distance[n.index()]);
        let mut counts = vec![BigUint::default(); self.graph.node_count()];
        counts[goal.index()] = BigUint::from(1u8);
        for node in order {
            if node == goal {
                continue;
            }
            for &next in &self.neighbors[node.index()] {
                if distance[next.index()].checked_add(1) == Some(distance[node.index()]) {
                    let next_count = counts[next.index()].clone();
                    counts[node.index()] += next_count;
                }
            }
        }
        struct PageWalk<'a> {
            graph: &'a TeammateGraph,
            goal: NodeIndex,
            distance: &'a [usize],
            counts: &'a [BigUint],
            skip: BigUint,
            limit: usize,
            paths: Vec<Vec<NodeIndex>>,
        }
        impl PageWalk<'_> {
            fn collect(&mut self, node: NodeIndex, path: &mut Vec<NodeIndex>) {
                if self.paths.len() >= self.limit {
                    return;
                }
                if self.skip >= self.counts[node.index()] {
                    self.skip -= &self.counts[node.index()];
                    return;
                }
                path.push(node);
                if node == self.goal {
                    self.paths.push(path.clone());
                } else {
                    for index in 0..self.graph.neighbors[node.index()].len() {
                        let next = self.graph.neighbors[node.index()][index];
                        if self.distance[next.index()].checked_add(1)
                            == Some(self.distance[node.index()])
                        {
                            self.collect(next, path);
                            if self.paths.len() >= self.limit {
                                break;
                            }
                        }
                    }
                }
                path.pop();
            }
        }
        let mut walk = PageWalk {
            graph: self,
            goal,
            distance: &distance,
            counts: &counts,
            skip: offset.clone(),
            limit,
            paths: Vec::new(),
        };
        walk.collect(start, &mut Vec::new());
        let paths = walk.paths;
        let exact_total = &counts[start.index()];
        let total = u64::try_from(exact_total).unwrap_or(u64::MAX);
        let saturated = exact_total > &BigUint::from(u64::MAX);
        let next = offset + BigUint::from(paths.len());
        let has_more = &next < exact_total && !paths.is_empty();
        let next_offset = if has_more {
            u64::try_from(&next).ok()
        } else {
            None
        };
        let next_cursor = if has_more {
            Some(format!("v1:{}", next.to_str_radix(10)))
        } else {
            None
        };
        let chains = paths
            .into_iter()
            .map(|path| ChainWithDegree {
                links: self.chain_links(&path),
                degree: distance[start.index()],
                path: path.into_iter().map(|n| self.graph[n].id.clone()).collect(),
            })
            .collect();
        Some(ChainPage {
            chains,
            total,
            total_exact: exact_total.to_str_radix(10),
            total_saturated: saturated,
            next_offset,
            next_cursor,
        })
    }

    fn distances(&self, start: NodeIndex) -> Vec<usize> {
        let mut distances = vec![usize::MAX; self.graph.node_count()];
        let mut queue = Vec::with_capacity(self.graph.node_count());
        distances[start.index()] = 0;
        queue.push(start);
        let mut cursor = 0;
        while cursor < queue.len() {
            let node = queue[cursor];
            cursor += 1;
            for &next in &self.neighbors[node.index()] {
                if distances[next.index()] == usize::MAX {
                    distances[next.index()] = distances[node.index()] + 1;
                    queue.push(next);
                }
            }
        }
        distances
    }

    /// Exact unordered-pair statistics, cached for this immutable graph.
    /// Each BFS uses dense indices; only one distance row is retained.
    pub fn statistics(&self) -> GraphStatistics {
        self.statistics
            .get_or_init(|| {
                let n = self.graph.node_count();
                let mut histogram = BTreeMap::new();
                let mut unreachable_pairs = 0;
                let mut components = 0;
                let mut component_seen = vec![false; n];
                for start in self.graph.node_indices() {
                    let distances = self.distances(start);
                    if !component_seen[start.index()] {
                        components += 1;
                        for (index, &distance) in distances.iter().enumerate() {
                            if distance != usize::MAX {
                                component_seen[index] = true;
                            }
                        }
                    }
                    for &distance in distances.iter().skip(start.index() + 1) {
                        if distance == usize::MAX {
                            unreachable_pairs += 1;
                        } else {
                            *histogram.entry(distance).or_insert(0) += 1;
                        }
                    }
                }
                let diameter = histogram.keys().copied().max().unwrap_or(0);
                GraphStatistics {
                    players: n,
                    histogram,
                    components,
                    diameter,
                    unreachable_pairs,
                }
            })
            .clone()
    }
}

/// Graph-wide separation statistics (spec: histogram, components, diameter).
#[derive(Clone, Debug)]
pub struct GraphStatistics {
    pub players: usize,
    /// Reachable unordered pairs by shortest-path length: length -> pair count.
    pub histogram: BTreeMap<usize, usize>,
    /// Number of connected components.
    pub components: usize,
    /// Maximum finite shortest-path length within connected components.
    pub diameter: usize,
    /// Unordered player pairs with no teammate chain between them.
    pub unreachable_pairs: usize,
}

/// Bounded shortest-chain result, suitable for a browser query.
#[derive(Clone, Debug)]
pub struct ChainPage {
    pub chains: Vec<ChainWithDegree>,
    /// Legacy numeric count, capped when it cannot fit u64.
    pub total: u64,
    /// Exact decimal count; consume as text to avoid JSON number precision loss.
    pub total_exact: String,
    pub total_saturated: bool,
    /// Legacy numeric continuation, when the next rank fits u64.
    pub next_offset: Option<u64>,
    /// Resumable continuation for every rank, including beyond integer limits.
    pub next_cursor: Option<String>,
}
