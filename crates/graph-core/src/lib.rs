//! Fixture-backed teammate graph.
//!
//! Domain rules (spec, Implementation Decisions):
//! - Nodes are player nodes, identified independently of display names.
//! - A teammate edge requires a positive time overlap of roster tenures on the
//!   same team. Same-franchise, non-overlapping tenures do not create an edge.
//! - A player's tenures are independent intervals; a stint only links them to
//!   players overlapping that stint.
//! - Repeated overlaps between the same pair collapse to one undirected
//!   teammate edge whose provenance accumulates all overlapping stints.

use std::collections::{BTreeMap, HashMap, HashSet};

use petgraph::graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoNodeIdentifiers, VisitMap, Visitable};
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
        for (team, team_tenures) in by_team {
            for (i, a) in team_tenures.iter().enumerate() {
                for b in team_tenures.iter().skip(i + 1) {
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
        TeammateGraph {
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
            for neighbor in self.graph.neighbors_undirected(node) {
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
            for neighbor in self.graph.neighbors_undirected(node) {
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

    /// Graph statistics: connected components, the separation histogram over
    /// reachable unordered pairs, diameter within components, and the count of
    /// unreachable pairs (reported separately, never folded into the diameter).
    pub fn statistics(&self) -> GraphStatistics {
        let ids = self.player_ids();
        let n = ids.len();
        let index_of: HashMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        // All-pairs shortest distances via BFS from each node.
        let mut distances: Vec<Vec<Option<usize>>> = vec![Vec::new(); n];
        for (i, id) in ids.iter().enumerate() {
            let start = self.by_id[id];
            let mut dist: HashMap<NodeIndex, usize> = HashMap::new();
            dist.insert(start, 0);
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            while let Some(node) = queue.pop_front() {
                let d = dist[&node];
                for neighbor in self.graph.neighbors_undirected(node) {
                    if !dist.contains_key(&neighbor) {
                        dist.insert(neighbor, d + 1);
                        queue.push_back(neighbor);
                    }
                }
            }
            let mut row = vec![None; n];
            for (node, d) in dist {
                let player = &self.graph[node].id;
                row[index_of[player.as_str()]] = Some(d);
            }
            distances[i] = row;
        }
        let mut histogram: BTreeMap<usize, usize> = BTreeMap::new();
        let mut unreachable_pairs = 0usize;
        for i in 0..n {
            for j in (i + 1)..n {
                match distances[i][j] {
                    Some(d) => *histogram.entry(d).or_insert(0) += 1,
                    None => unreachable_pairs += 1,
                }
            }
        }
        let components = self.connected_components(&ids);
        let diameter = histogram.keys().copied().max().unwrap_or(0);
        GraphStatistics {
            players: n,
            histogram,
            components,
            diameter,
            unreachable_pairs,
        }
    }

    fn connected_components(&self, ids: &[String]) -> usize {
        let mut seen: HashSet<usize> = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut components = 0;
        for id in ids {
            let start = self
                .graph
                .node_identifiers()
                .find(|n| self.graph[*n].id == *id)
                .expect("player node exists");
            if seen.contains(&start.index()) {
                continue;
            }
            components += 1;
            queue.push_back(start);
            seen.insert(start.index());
            while let Some(node) = queue.pop_front() {
                for neighbor in self.graph.neighbors_undirected(node) {
                    if seen.insert(neighbor.index()) {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        components
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
