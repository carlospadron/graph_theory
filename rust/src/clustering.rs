use crate::{BuiltGraph, Node, Edge};
use petgraph::graph::NodeIndex;
use std::collections::{HashMap, BinaryHeap};
use petgraph::visit::EdgeRef;
use std::cmp::Ordering;

#[derive(Copy, Clone, PartialEq)]
struct State {
    cost: f64,
    position: NodeIndex,
}

impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn edge_weight(e: &Edge) -> f64 {
    match e {
        Edge::Road { length_m, .. } => *length_m,
        Edge::BuildingAccess { .. } => 0.0,
    }
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n).collect() }
    }
    fn find(&mut self, i: usize) -> usize {
        if self.parent[i] == i {
            i
        } else {
            let p = self.parent[i];
            let root = self.find(p);
            self.parent[i] = root;
            root
        }
    }
    fn union(&mut self, i: usize, j: usize) {
        let root_i = self.find(i);
        let root_j = self.find(j);
        if root_i != root_j {
            self.parent[root_i] = root_j;
        }
    }
}

pub fn cluster_buildings(graph: &BuiltGraph, max_distance: f64) -> HashMap<NodeIndex, usize> {
    let mut building_to_idx: HashMap<NodeIndex, usize> = HashMap::new();
    let mut idx_to_building: Vec<NodeIndex> = Vec::new();

    // Map each building to a contiguous index 0..N for the UnionFind structure
    for &b in graph.building_nodes.values() {
        building_to_idx.insert(b, idx_to_building.len());
        idx_to_building.push(b);
    }

    let n = idx_to_building.len();
    let mut uf = UnionFind::new(n);

    // Single linkage: Two buildings are in the same cluster if their shortest path distance
    // on the graph is <= max_distance. By running Dijkstra from every building and linking
    // any found buildings, we naturally compute the exact connected components for the threshold.
    for i in 0..n {
        let start_building = idx_to_building[i];

        let mut dist: HashMap<NodeIndex, f64> = HashMap::new();
        let mut heap = BinaryHeap::new();

        dist.insert(start_building, 0.0);
        heap.push(State { cost: 0.0, position: start_building });

        while let Some(State { cost, position }) = heap.pop() {
            if cost > max_distance {
                continue; // Stop expanding beyond max_distance
            }
            if cost > *dist.get(&position).unwrap_or(&f64::INFINITY) {
                continue;
            }

            if let Node::Building { .. } = graph.graph[position] {
                if position != start_building {
                    if let Some(&j) = building_to_idx.get(&position) {
                        // Merge the cluster of start_building with the cluster of found building
                        uf.union(i, j);
                    }
                }
            }

            for edge in graph.graph.edges(position) {
                let next = edge.target();
                let next_node = if next == position { edge.source() } else { next };
                let next_cost = cost + edge_weight(edge.weight());

                if next_cost <= max_distance {
                    let is_better = dist.get(&next_node).map_or(true, |&c| next_cost < c);
                    if is_better {
                        dist.insert(next_node, next_cost);
                        heap.push(State { cost: next_cost, position: next_node });
                    }
                }
            }
        }
    }

    // Extract clusters and flatten the IDs to be sequential (0, 1, 2...)
    let mut building_clusters = HashMap::new();
    let mut root_to_cluster_id = HashMap::new();
    let mut next_cluster_id = 0;

    for i in 0..n {
        let root = uf.find(i);
        let cluster_id = *root_to_cluster_id.entry(root).or_insert_with(|| {
            let id = next_cluster_id;
            next_cluster_id += 1;
            id
        });
        building_clusters.insert(idx_to_building[i], cluster_id);
    }

    building_clusters
}
