use crate::{BuiltGraph, Edge};
use petgraph::algo::dijkstra;
use petgraph::graph::NodeIndex;

fn edge_weight(e: &Edge) -> f64 {
    match e {
        Edge::Road { length_m, .. } => *length_m,
        Edge::BuildingAccess { distance_m } => *distance_m,
    }
}

/// Prim's MST on an n×n distance matrix. Returns total MST weight.
/// O(n²) — fast enough for the metric closure of a terminal set.
fn prim_mst(dist: &[Vec<f64>]) -> f64 {
    let n = dist.len();
    if n < 2 {
        return 0.0;
    }
    let mut in_mst = vec![false; n];
    let mut key = vec![f64::INFINITY; n];
    key[0] = 0.0;
    let mut total = 0.0;
    for _ in 0..n {
        let u = (0..n)
            .filter(|&i| !in_mst[i])
            .min_by(|&a, &b| key[a].partial_cmp(&key[b]).unwrap())
            .unwrap();
        in_mst[u] = true;
        total += key[u];
        for v in 0..n {
            if !in_mst[v] && dist[u][v] < key[v] {
                key[v] = dist[u][v];
            }
        }
    }
    total
}

/// Approximate Steiner tree weight for a terminal set.
///
/// Uses the standard 2-approximation: compute all-pairs shortest paths between
/// terminals (Dijkstra), build the metric closure, then find its MST (Prim).
/// The result is within a factor of 2 of the optimal Steiner tree weight.
pub fn steiner_weight(graph: &BuiltGraph, terminals: &[NodeIndex]) -> f64 {
    let n = terminals.len();
    if n < 2 {
        return 0.0;
    }

    let mut dist = vec![vec![f64::INFINITY; n]; n];
    for (i, &src) in terminals.iter().enumerate() {
        dist[i][i] = 0.0;
        let sp = dijkstra(&graph.graph, src, None, |e| edge_weight(e.weight()));
        for (j, &dst) in terminals.iter().enumerate() {
            if let Some(&d) = sp.get(&dst) {
                dist[i][j] = d;
            }
        }
    }

    prim_mst(&dist)
}

/// A solution on the Pareto front: how many terminals were selected and the
/// approximate Steiner tree weight connecting them.
#[derive(Debug, Clone)]
pub struct Solution {
    pub selected: Vec<NodeIndex>,
    pub tree_weight: f64,
}

/// Brute-force Pareto search over all non-empty subsets of `candidates`.
///
/// For each subset it computes the Steiner tree weight and keeps only
/// non-dominated solutions (no other subset has both more nodes AND lower
/// weight). Returns the front sorted by descending node count.
///
/// Complexity: O(2^n · n² · V log V) — only feasible for n ≤ ~20.
pub fn brute_force(graph: &BuiltGraph, candidates: &[NodeIndex]) -> Vec<Solution> {
    let n = candidates.len();
    assert!(n <= 20, "brute_force is only feasible for ≤ 20 candidates");

    let mut pareto: Vec<Solution> = Vec::new();

    for mask in 1u64..(1u64 << n) {
        let selected: Vec<NodeIndex> = (0..n)
            .filter(|&i| mask & (1 << i) != 0)
            .map(|i| candidates[i])
            .collect();

        let weight = steiner_weight(graph, &selected);

        let dominated = pareto
            .iter()
            .any(|p| p.selected.len() >= selected.len() && p.tree_weight <= weight);

        if !dominated {
            pareto.retain(|p| {
                !(selected.len() >= p.selected.len() && weight <= p.tree_weight)
            });
            pareto.push(Solution { selected, tree_weight: weight });
        }
    }

    pareto.sort_by(|a, b| b.selected.len().cmp(&a.selected.len()));
    pareto
}

/// Greedy Pareto approximation.
///
/// Starting from an empty selection, at each step adds the candidate that
/// produces the smallest marginal increase in Steiner tree weight. Returns
/// one solution per step (from 1 selected node up to all candidates), giving
/// a greedy approximation of the Pareto front.
///
/// Complexity: O(n² · n · V log V) — practical for thousands of candidates.
pub fn greedy_pareto(graph: &BuiltGraph, candidates: &[NodeIndex]) -> Vec<Solution> {
    let mut remaining: Vec<NodeIndex> = candidates.to_vec();
    let mut selected: Vec<NodeIndex> = Vec::new();
    let mut solutions: Vec<Solution> = Vec::new();

    while !remaining.is_empty() {
        let (best_i, best_w) = remaining
            .iter()
            .enumerate()
            .map(|(i, &node)| {
                let mut trial = selected.clone();
                trial.push(node);
                (i, steiner_weight(graph, &trial))
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        selected.push(remaining.swap_remove(best_i));
        solutions.push(Solution {
            selected: selected.clone(),
            tree_weight: best_w,
        });
    }

    solutions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Connector, RoadConnectorRef, RoadSegment, build_graph};

    /// Three connectors in a line: c1 –(10m)– c2 –(10m)– c3
    /// Candidates: c1 and c3 (endpoints).
    /// Steiner tree must route through c2, so weight = 20m.
    fn line_graph() -> BuiltGraph {
        let roads = vec![
            RoadSegment {
                id: "r1".into(),
                length_m: 10.0,
                connectors: vec![
                    RoadConnectorRef { connector_id: "c1".into(), at: 0.0 },
                    RoadConnectorRef { connector_id: "c2".into(), at: 1.0 },
                ],
            },
            RoadSegment {
                id: "r2".into(),
                length_m: 10.0,
                connectors: vec![
                    RoadConnectorRef { connector_id: "c2".into(), at: 0.0 },
                    RoadConnectorRef { connector_id: "c3".into(), at: 1.0 },
                ],
            },
        ];
        let connectors = vec![
            Connector { id: "c1".into(), x: 0.0, y: 0.0 },
            Connector { id: "c2".into(), x: 1.0, y: 0.0 },
            Connector { id: "c3".into(), x: 2.0, y: 0.0 },
        ];
        build_graph(&roads, &connectors, &[], &[]).unwrap()
    }

    #[test]
    fn steiner_weight_routes_through_intermediate_node() {
        let g = line_graph();
        let c1 = g.connector_nodes["c1"];
        let c3 = g.connector_nodes["c3"];
        let w = steiner_weight(&g, &[c1, c3]);
        assert!((w - 20.0).abs() < 1e-9, "expected 20m, got {w}");
    }

    #[test]
    fn brute_force_returns_pareto_front() {
        let g = line_graph();
        let candidates: Vec<NodeIndex> = ["c1", "c2", "c3"]
            .iter()
            .map(|id| g.connector_nodes[*id])
            .collect();

        let front = brute_force(&g, &candidates);
        // All three selected must appear somewhere on the front
        assert!(front.iter().any(|s| s.selected.len() == 3));
        // Pareto: no solution is dominated
        for i in 0..front.len() {
            for j in 0..front.len() {
                if i == j { continue; }
                let a = &front[i];
                let b = &front[j];
                assert!(
                    !(b.selected.len() >= a.selected.len() && b.tree_weight <= a.tree_weight),
                    "solution {i} is dominated by {j}"
                );
            }
        }
    }

    #[test]
    fn greedy_pareto_produces_one_solution_per_step() {
        let g = line_graph();
        let candidates: Vec<NodeIndex> = ["c1", "c2", "c3"]
            .iter()
            .map(|id| g.connector_nodes[*id])
            .collect();

        let solutions = greedy_pareto(&g, &candidates);
        assert_eq!(solutions.len(), 3);
        for (i, s) in solutions.iter().enumerate() {
            assert_eq!(s.selected.len(), i + 1);
        }
        // Weights must be non-decreasing as we add more nodes
        for w in solutions.windows(2) {
            assert!(w[1].tree_weight >= w[0].tree_weight);
        }
    }

    #[test]
    fn steiner_weight_handles_single_node() {
        let g = line_graph();
        let c1 = g.connector_nodes["c1"];
        let w = steiner_weight(&g, &[c1]);
        assert!((w - 0.0).abs() < 1e-9, "single node should have weight 0, got {w}");
    }

    #[test]
    fn steiner_weight_handles_two_nodes() {
        let g = line_graph();
        let c1 = g.connector_nodes["c1"];
        let c2 = g.connector_nodes["c2"];
        let w = steiner_weight(&g, &[c1, c2]);
        assert!((w - 10.0).abs() < 1e-9, "expected 10m, got {w}");
    }

    #[test]
    fn greedy_pareto_handles_single_candidate() {
        let g = line_graph();
        let c1 = g.connector_nodes["c1"];
        let solutions = greedy_pareto(&g, &[c1]);
        assert_eq!(solutions.len(), 1);
        assert_eq!(solutions[0].selected.len(), 1);
        assert!((solutions[0].tree_weight - 0.0).abs() < 1e-9);
    }

    #[test]
    fn brute_force_handles_two_candidates() {
        let g = line_graph();
        let c1 = g.connector_nodes["c1"];
        let c2 = g.connector_nodes["c2"];
        let solutions = brute_force(&g, &[c1, c2]);
        assert!(solutions.len() > 0);
        // Should have solutions for {c1}, {c2}, and {c1, c2}
        assert!(solutions.iter().any(|s| s.selected.len() == 1));
        assert!(solutions.iter().any(|s| s.selected.len() == 2));
    }
}

#[derive(Debug, Clone)]
pub struct ClusterSolution {
    pub selected_clusters: Vec<usize>,
    pub total_buildings_yield: usize,
    pub tree_weight: f64,
}

/// Cluster-Level Greedy Pareto Search
/// 
/// Instead of evaluating individual buildings, this evaluates entire clusters.
/// At each step, it adds the cluster that provides the best ratio of:
/// (Cost to connect the cluster hub to the existing tree) / (Number of buildings in the cluster)
/// 
/// `clusters_map` is a map from cluster_id to a list of building NodeIndexes.
pub fn greedy_cluster_pareto(
    graph: &BuiltGraph,
    clusters_map: &std::collections::HashMap<usize, Vec<NodeIndex>>,
) -> Vec<ClusterSolution> {
    let mut remaining_clusters: Vec<usize> = clusters_map.keys().copied().collect();
    let mut selected_centers: Vec<NodeIndex> = Vec::new();
    let mut selected_clusters: Vec<usize> = Vec::new();
    let mut solutions: Vec<ClusterSolution> = Vec::new();
    let mut total_buildings = 0;

    // Pick a "hub" for each cluster. For simplicity, we just take the first building in the list.
    let mut cluster_hubs: std::collections::HashMap<usize, NodeIndex> = std::collections::HashMap::new();
    for (&id, nodes) in clusters_map {
        if let Some(&hub) = nodes.first() {
            cluster_hubs.insert(id, hub);
        }
    }

    while !remaining_clusters.is_empty() {
        let (best_idx, best_w, best_yield) = remaining_clusters
            .iter()
            .enumerate()
            .map(|(i, &cluster_id)| {
                let hub = cluster_hubs[&cluster_id];
                let b_count = clusters_map[&cluster_id].len();

                let mut trial = selected_centers.clone();
                trial.push(hub);
                let new_weight = steiner_weight(graph, &trial);
                
                // We want the lowest marginal cost per building yield
                // If it's the first step, previous weight is 0.
                let prev_weight = if solutions.is_empty() { 0.0 } else { solutions.last().unwrap().tree_weight };
                let marginal_cost = new_weight - prev_weight;
                
                let ratio = marginal_cost / (b_count as f64);
                
                (i, new_weight, b_count, ratio)
            })
            // Min by ratio
            .min_by(|a, b| a.3.partial_cmp(&b.3).unwrap())
            .map(|(i, w, y, _)| (i, w, y))
            .unwrap();

        let chosen_cluster = remaining_clusters.swap_remove(best_idx);
        let chosen_hub = cluster_hubs[&chosen_cluster];

        selected_centers.push(chosen_hub);
        selected_clusters.push(chosen_cluster);
        total_buildings += best_yield;

        solutions.push(ClusterSolution {
            selected_clusters: selected_clusters.clone(),
            total_buildings_yield: total_buildings,
            tree_weight: best_w,
        });
    }

    solutions
}
