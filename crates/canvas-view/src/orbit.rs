//! Deterministic Orbit geometry and exact discovery over the admitted graph.
use crate::{GraphLink, GraphNode};
use std::collections::{HashMap, VecDeque};

fn hash(id: &str, seed: u32) -> f64 {
    let mut h = id
        .bytes()
        .fold(seed, |h, b| (h ^ u32::from(b)).wrapping_mul(16777619));
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb352d);
    h ^= h >> 15;
    f64::from(h) / f64::from(u32::MAX)
}

/// The chain occupies the inner arc; all other players move into a stable halo.
/// No pair layout relies on names, graph size or a particular evidence snapshot.
pub fn layout(
    nodes: &[GraphNode],
    links: &[GraphLink],
    focus: &str,
    path: &[String],
) -> Vec<(f64, f64)> {
    let direct: std::collections::HashSet<_> = links
        .iter()
        .filter_map(|e| {
            if e.from == focus {
                Some(e.to.as_str())
            } else if e.to == focus {
                Some(e.from.as_str())
            } else {
                None
            }
        })
        .collect();
    let mut neighbors: Vec<_> = nodes
        .iter()
        .filter(|n| direct.contains(n.id.as_str()))
        .map(|n| n.id.as_str())
        .collect();
    neighbors.sort_unstable();
    nodes
        .iter()
        .map(|n| {
            if let Some(k) = path.iter().position(|id| id == &n.id) {
                if path.len() == 1 {
                    return (0.0, 0.0);
                }
                let angle = -2.6 + k as f64 / (path.len() - 1) as f64 * 5.2;
                return (angle.cos() * 440.0, angle.sin() * 440.0);
            }
            if path.is_empty() && !focus.is_empty() {
                if n.id == focus {
                    return (0.0, 0.0);
                }
                if let Ok(k) = neighbors.binary_search(&n.id.as_str()) {
                    let angle = k as f64 / neighbors.len().max(1) as f64 * std::f64::consts::TAU;
                    let radius = 360.0 + (k % 2) as f64 * 95.0;
                    return (radius * angle.cos(), radius * angle.sin());
                }
            }
            let angle = hash(&n.id, 2166136261) * std::f64::consts::TAU;
            let radius = if path.is_empty() && focus.is_empty() {
                150.0 + hash(&n.id, 37).sqrt() * 800.0
            } else {
                850.0 + hash(&n.id, 37) * 220.0
            };
            (radius * angle.cos(), radius * angle.sin())
        })
        .collect()
}

pub fn adjacency(nodes: &[GraphNode], links: &[GraphLink]) -> Vec<Vec<usize>> {
    let indices: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut adj = vec![Vec::new(); nodes.len()];
    for e in links {
        if let (Some(&a), Some(&b)) = (indices.get(e.from.as_str()), indices.get(e.to.as_str())) {
            adj[a].push(b);
            adj[b].push(a);
        }
    }
    for row in &mut adj {
        row.sort_unstable();
        row.dedup();
    }
    adj
}

pub fn distances(adj: &[Vec<usize>], root: usize) -> Vec<usize> {
    let mut result = vec![usize::MAX; adj.len()];
    result[root] = 0;
    let mut queue = VecDeque::from([root]);
    while let Some(a) = queue.pop_front() {
        for &b in &adj[a] {
            if result[b] == usize::MAX {
                result[b] = result[a] + 1;
                queue.push_back(b);
            }
        }
    }
    result
}

/// Exact top finite shortest distances; unordered pairs, stable ID tie ordering.
/// Retains only ten results and one BFS distance row at a time.
pub fn longest_pairs(nodes: &[GraphNode], adj: &[Vec<usize>]) -> Vec<(usize, usize, usize)> {
    let mut order: Vec<_> = (0..nodes.len()).collect();
    order.sort_by_key(|&i| &nodes[i].id);
    let mut best: Vec<(usize, usize, usize)> = Vec::new();
    for (rank, &a) in order.iter().enumerate() {
        let ds = distances(adj, a);
        for &b in &order[rank + 1..] {
            let d = ds[b];
            if d == usize::MAX || best.len() == 10 && d <= best[9].2 {
                continue;
            }
            best.push((a, b, d));
            best.sort_by(|&(a, b, d), &(x, y, e)| {
                e.cmp(&d)
                    .then_with(|| nodes[a].id.cmp(&nodes[x].id))
                    .then_with(|| nodes[b].id.cmp(&nodes[y].id))
            });
            best.truncate(10);
        }
    }
    best
}

/// Forward moves that preserve a shortest prefix, optionally to a fixed destination.
pub fn next_steps(
    adj: &[Vec<usize>],
    path: &[usize],
    root_distances: &[usize],
    target_distances: Option<&[usize]>,
) -> Vec<usize> {
    let Some(&current) = path.last() else {
        return Vec::new();
    };
    let total = target_distances.map(|d| d[path[0]]);
    adj[current]
        .iter()
        .copied()
        .filter(|&i| {
            root_distances[i] == path.len()
                && !path.contains(&i)
                && !path[..path.len() - 1]
                    .iter()
                    .any(|&prior| adj[prior].contains(&i))
                && target_distances.is_none_or(|d| {
                    d[i] != usize::MAX
                        && total.is_some_and(|total| {
                            total != usize::MAX && d[i].checked_add(path.len()) == Some(total)
                        })
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn nodes(n: usize) -> Vec<GraphNode> {
        (0..n)
            .map(|i| GraphNode {
                id: i.to_string(),
                name: i.to_string(),
                era: String::new(),
                teams: vec![],
                distance: None,
                debut_season: None,
                degree: 0,
            })
            .collect()
    }
    #[test]
    fn chain_clears_halo_and_same_player_stays_finite() {
        let ns = nodes(30);
        let p = vec!["1".into(), "5".into(), "9".into()];
        let points = layout(&ns, &[], "1", &p);
        for (i, point) in points.iter().enumerate() {
            let radius = point.0.hypot(point.1);
            if p.contains(&ns[i].id) {
                assert!(radius < 500.0);
            } else {
                assert!(radius >= 850.0);
            }
        }
        assert_eq!(layout(&ns, &[], "1", &["1".into()])[1], (0.0, 0.0));
        assert!(layout(&[], &[], "", &[]).is_empty());
    }
    #[test]
    fn exploration_rejects_global_detours_even_without_a_direct_back_edge() {
        // A six-cycle: 0→1→2→3 is shortest, but going on to 4 would be
        // a detour because 0→5→4 is only two hops. Earlier-neighbor
        // exclusion alone would incorrectly allow that move.
        let adj = vec![
            vec![1, 5],
            vec![0, 2],
            vec![1, 3],
            vec![2, 4],
            vec![3, 5],
            vec![0, 4],
            vec![],
        ];
        let d = distances(&adj, 0);
        assert_eq!(next_steps(&adj, &[0], &d, None), vec![1, 5]);
        assert_eq!(next_steps(&adj, &[0, 1], &d, None), vec![2]);
        assert_eq!(next_steps(&adj, &[0, 1, 2], &d, None), vec![3]);
        assert!(next_steps(&adj, &[0, 1, 2, 3], &d, None).is_empty());
        assert_eq!(next_steps(&adj, &[0, 5], &d, None), vec![4]);
        assert!(next_steps(&adj, &[], &d, None).is_empty());
        assert!(next_steps(&adj, &[6], &distances(&adj, 6), None).is_empty());
    }
    #[test]
    fn destination_layers_offer_only_moves_on_a_shortest_route() {
        let adj = vec![vec![1], vec![0, 2, 4], vec![1, 3], vec![2], vec![1], vec![]];
        let root = distances(&adj, 0);
        let end = distances(&adj, 3);
        assert_eq!(next_steps(&adj, &[0, 1], &root, None), vec![2, 4]);
        assert_eq!(next_steps(&adj, &[0, 1], &root, Some(&end)), vec![2]);
        assert!(next_steps(&adj, &[0, 1, 2, 3], &root, Some(&end)).is_empty());
        assert!(next_steps(&adj, &[0], &root, Some(&distances(&adj, 5))).is_empty());
        assert!(next_steps(&adj, &[0], &root, Some(&root)).is_empty());
    }
    #[test]
    fn longest_is_shortest_distance_not_a_detour_and_excludes_disconnected_pairs() {
        let ns = nodes(5);
        let adj = vec![vec![1, 3], vec![0, 2], vec![1, 3], vec![0, 2], vec![]];
        let pairs = longest_pairs(&ns, &adj);
        assert_eq!(pairs.len(), 6);
        assert_eq!(pairs[0], (0, 2, 2));
        assert_eq!(pairs[1], (1, 3, 2));
        assert!(pairs.iter().all(|&(a, b, d)| a < 4 && b < 4 && d <= 2));
        assert!(longest_pairs(&[], &[]).is_empty());
    }
}
