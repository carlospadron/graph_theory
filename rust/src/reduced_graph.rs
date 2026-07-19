use crate::{BuiltGraph, Node, Edge};
use petgraph::graph::{Graph, NodeIndex};
use petgraph::Undirected;
use std::collections::{HashMap, BinaryHeap};
use std::cmp::Ordering;
use petgraph::visit::EdgeRef;

#[derive(Copy, Clone, PartialEq)]
struct DijkstraState {
    cost: f64,
    node: NodeIndex,
    cluster_id: usize,
}

impl Eq for DijkstraState {}

impl Ord for DijkstraState {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for DijkstraState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn edge_weight(e: &Edge) -> f64 {
    match e {
        Edge::Road { length_m, .. } => *length_m,
        Edge::BuildingAccess { distance_m } => *distance_m,
    }
}

#[derive(Debug, Clone)]
pub struct ReducedClusterNode {
    pub cluster_id: usize,
    pub size: usize,
    pub centroid_x: f64,
    pub centroid_y: f64,
}

#[derive(Debug, Clone)]
pub struct ReducedClusterEdge {
    pub distance_m: f64,
}

pub type ReducedGraph = Graph<ReducedClusterNode, ReducedClusterEdge, Undirected>;

/// Build a coarsened "Reduced Graph" of clusters.
/// 
/// 1. Finds the centroid coordinates for each cluster.
/// 2. Performs a multi-source Dijkstra starting from all buildings assigned to their clusters.
/// 3. Records the nearest cluster and distance for every node/connector in the original graph.
/// 4. Finds edges where two different cluster "frontiers" meet. The sum of the distances 
///    to each cluster plus the edge weight gives the exact shortest road distance between 
///    the cluster boundaries.
/// 5. Constructs a clean, sparse neighboring-cluster graph.
pub fn build_reduced_graph(
    graph: &BuiltGraph,
    building_clusters: &HashMap<NodeIndex, usize>,
) -> ReducedGraph {
    let mut clusters_map: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
    for (&node_idx, &cluster_id) in building_clusters {
        clusters_map.entry(cluster_id).or_default().push(node_idx);
    }

    // Step 1: Compute cluster nodes and centroids
    let mut cluster_id_to_index = HashMap::new();
    let mut new_graph = Graph::new_undirected();

    let mut sorted_cluster_ids: Vec<usize> = clusters_map.keys().copied().collect();
    sorted_cluster_ids.sort();

    for cid in sorted_cluster_ids {
        let nodes = &clusters_map[&cid];
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut count = 0;

        for &node_idx in nodes {
            if let Node::Building { x, y, .. } = &graph.graph[node_idx] {
                sum_x += x;
                sum_y += y;
                count += 1;
            }
        }

        let centroid_x = if count > 0 { sum_x / count as f64 } else { 0.0 };
        let centroid_y = if count > 0 { sum_y / count as f64 } else { 0.0 };

        let node_data = ReducedClusterNode {
            cluster_id: cid,
            size: count,
            centroid_x,
            centroid_y,
        };
        let new_idx = new_graph.add_node(node_data);
        cluster_id_to_index.insert(cid, new_idx);
    }

    // Step 2: Multi-source Dijkstra from all buildings
    let mut dist_map: HashMap<NodeIndex, (usize, f64)> = HashMap::new();
    let mut heap = BinaryHeap::new();

    for (&node_idx, &cluster_id) in building_clusters {
        dist_map.insert(node_idx, (cluster_id, 0.0));
        heap.push(DijkstraState { cost: 0.0, node: node_idx, cluster_id });
    }

    while let Some(DijkstraState { cost, node, cluster_id }) = heap.pop() {
        if let Some(&(nearest_cid, recorded_dist)) = dist_map.get(&node) {
            if nearest_cid == cluster_id && cost > recorded_dist {
                continue;
            }
        }

        for edge in graph.graph.edges(node) {
            let next = edge.target();
            let next_node = if next == node { edge.source() } else { next };
            let next_cost = cost + edge_weight(edge.weight());

            let should_update = match dist_map.get(&next_node) {
                None => true,
                Some(&(_, current_dist)) => next_cost < current_dist,
            };

            if should_update {
                dist_map.insert(next_node, (cluster_id, next_cost));
                heap.push(DijkstraState { cost: next_cost, node: next_node, cluster_id });
            }
        }
    }

    // Step 3: Find boundary crossings between different clusters
    let mut boundary_edges: HashMap<(usize, usize), f64> = HashMap::new();

    for edge in graph.graph.edge_references() {
        let u = edge.source();
        let v = edge.target();

        if let (Some(&(cid_u, dist_u)), Some(&(cid_v, dist_v))) = (dist_map.get(&u), dist_map.get(&v)) {
            if cid_u != cid_v {
                let road_dist = dist_u + edge_weight(edge.weight()) + dist_v;
                let key = if cid_u < cid_v { (cid_u, cid_v) } else { (cid_v, cid_u) };

                boundary_edges.entry(key)
                    .and_modify(|existing| if road_dist < *existing { *existing = road_dist; })
                    .or_insert(road_dist);
            }
        }
    }

    // Step 4: Add boundary edges to our coarsened graph
    for ((cid_a, cid_b), dist) in boundary_edges {
        if let (Some(&idx_a), Some(&idx_b)) = (cluster_id_to_index.get(&cid_a), cluster_id_to_index.get(&cid_b)) {
            new_graph.add_edge(idx_a, idx_b, ReducedClusterEdge { distance_m: dist });
        }
    }

    new_graph
}
