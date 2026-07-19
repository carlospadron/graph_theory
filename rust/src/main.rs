use graph_builder::{io, clustering, reduced_graph, optimizer};
use std::collections::HashMap;
use std::path::Path;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::BinaryHeap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data = Path::new("../data");

    let connectors = io::read_connectors(data.join("rust_connectors.csv"))?;
    println!("Connectors loaded:      {:>7}", connectors.len());

    let buildings = io::read_buildings(data.join("rust_buildings.csv"))?;
    println!("Buildings loaded:       {:>7}", buildings.len());

    let connector_positions: HashMap<String, (f64, f64)> = connectors
        .iter()
        .map(|c| (c.id.clone(), (c.x, c.y)))
        .collect();

    let roads = io::read_roads(
        data.join("rust_road_connector_refs.csv"),
        &connector_positions,
    )?;
    println!("Road segments loaded:   {:>7}", roads.len());

    let links = io::read_building_links(data.join("rust_building_links.csv"))?;
    println!("Building links loaded:  {:>7}", links.len());

    println!("\nBuilding undirected graph...");
    let built = graph_builder::build_graph(&roads, &connectors, &buildings, &links)?;
    println!(
        "  nodes: {}   edges: {}",
        built.graph.node_count(),
        built.graph.edge_count()
    );

    // Run clustering
    println!("\nRunning building clustering (max distance: 100m)...");
    let clusters = clustering::cluster_buildings(&built, 100.0);
    println!("Created {} clusters", clusters.values().max().unwrap_or(&0) + 1);

    // Save clusters to CSV
    println!("Saving clusters to data/rust_clusters.csv...");
    let mut wtr = csv::Writer::from_path(data.join("rust_clusters.csv"))?;
    wtr.write_record(&["building_id", "cluster_id"])?;
    for (node_idx, cluster_id) in &clusters {
        if let graph_builder::Node::Building { id, .. } = &built.graph[*node_idx] {
            wtr.write_record(&[id, &cluster_id.to_string()])?;
        }
    }
    wtr.flush()?;
    println!("Clusters saved successfully.");

    // Build Reduced Graph
    println!("\nConstructing Coarsened Reduced Graph (using boundary-to-boundary wavefront matching)...");
    let r_graph = reduced_graph::build_reduced_graph(&built, &clusters);
    println!(
        "Reduced Graph created!\n  nodes (clusters): {}   edges (boundary connections): {}",
        r_graph.node_count(),
        r_graph.edge_count()
    );

    // Run optimizer directly on the Reduced Graph
    println!("\nRunning Optimizer directly on the Reduced Graph...");
    
    // Sort all cluster nodes in the reduced graph by size
    let mut r_nodes: Vec<NodeIndex> = r_graph.node_indices().collect();
    r_nodes.sort_by(|&a, &b| r_graph[b].size.cmp(&r_graph[a].size));

    // We will optimize over the top 15 largest cluster nodes on the reduced graph directly
    let demo_size = 15;
    let candidate_nodes: Vec<NodeIndex> = r_nodes.into_iter().take(demo_size).collect();

    println!("(Optimizing top {} largest nodes natively on the Reduced Graph)...", candidate_nodes.len());
    let solutions = optimizer::greedy_reduced_graph_pareto(&r_graph, &candidate_nodes);

    if !solutions.is_empty() {
        let last = solutions.last().unwrap();
        println!("  Optimized Solution: {} clusters selected, {} buildings yielded, {:.1} m tree weight", 
            last.selected_cluster_ids.len(), last.total_buildings_yield, last.tree_weight);

        // Save selected building IDs
        println!("\nSaving selected building IDs to data/selected_buildings.csv...");
        let mut target_wtr = csv::Writer::from_path(data.join("selected_buildings.csv"))?;
        target_wtr.write_record(&["building_id", "cluster_id"])?;

        let mut selected_cluster_ids = std::collections::HashSet::new();
        for &cid in &last.selected_cluster_ids {
            selected_cluster_ids.insert(cid);
        }

        for (node_idx, &cid) in &clusters {
            if selected_cluster_ids.contains(&cid) {
                if let graph_builder::Node::Building { id, .. } = &built.graph[*node_idx] {
                    target_wtr.write_record(&[id, &cid.to_string()])?;
                }
            }
        }
        target_wtr.flush()?;
        println!("Selected buildings saved successfully.");

        // Reconstruct paths natively from the Reduced Graph's boundary-to-boundary edges
        // using the full-detailed road network's geometry.
        println!("\nReconstructing exact road-routed path lines for the selected clusters...");
        let mut routing_road_segments: std::collections::HashSet<String> = std::collections::HashSet::new();
        
        let mut selected_hubs = Vec::new();
        for &cid in &last.selected_cluster_ids {
            // Find a building in this cluster to serve as a connection point
            if let Some((&node_idx, _)) = clusters.iter().find(|(_, &val)| val == cid) {
                selected_hubs.push(node_idx);
            }
        }

        // Run Dijkstra pairwise across selected clusters on the detailed graph to identify road segments
        if selected_hubs.len() > 1 {
            let mut visited_mst = std::collections::HashSet::new();
            visited_mst.insert(selected_hubs[0]);

            for &target_hub in &selected_hubs[1..] {
                let mut dist: HashMap<NodeIndex, f64> = HashMap::new();
                let mut parent_edge: HashMap<NodeIndex, (NodeIndex, String)> = HashMap::new(); // child -> (parent, road_segment_id)
                let mut heap = BinaryHeap::new();

                #[derive(Copy, Clone, PartialEq)]
                struct State { cost: f64, node: NodeIndex }
                impl Eq for State {}
                impl Ord for State {
                    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                        other.cost.partial_cmp(&self.cost).unwrap_or(std::cmp::Ordering::Equal)
                    }
                }
                impl PartialOrd for State {
                    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                        Some(self.cmp(other))
                    }
                }

                dist.insert(target_hub, 0.0);
                heap.push(State { cost: 0.0, node: target_hub });

                let mut reached_mst_node = None;
                while let Some(State { cost, node }) = heap.pop() {
                    if visited_mst.contains(&node) {
                        reached_mst_node = Some(node);
                        break;
                    }
                    if cost > *dist.get(&node).unwrap_or(&f64::INFINITY) {
                        continue;
                    }
                    for edge in built.graph.edges(node) {
                        let next = edge.target();
                        let next_node = if next == node { edge.source() } else { next };
                        
                        let (weight, segment_id) = match edge.weight() {
                            graph_builder::Edge::Road { length_m, segment_id } => (*length_m, segment_id.clone()),
                            graph_builder::Edge::BuildingAccess { distance_m } => (*distance_m, "".to_string()),
                        };
                        let next_cost = cost + weight;
                        let is_better = dist.get(&next_node).map_or(true, |&c| next_cost < c);
                        if is_better {
                            dist.insert(next_node, next_cost);
                            parent_edge.insert(next_node, (node, segment_id));
                            heap.push(State { cost: next_cost, node: next_node });
                        }
                    }
                }

                if let Some(mut curr) = reached_mst_node {
                    visited_mst.insert(curr);
                    while let Some((p, segment_id)) = parent_edge.get(&curr) {
                        if !segment_id.is_empty() {
                            routing_road_segments.insert(segment_id.clone());
                        }
                        visited_mst.insert(*p);
                        curr = *p;
                    }
                }
            }
        }

        // Save selected road segment IDs to a CSV
        println!("Saving active road segment IDs to data/optimized_tree_segments.csv...");
        let mut segment_wtr = csv::Writer::from_path(data.join("optimized_tree_segments.csv"))?;
        segment_wtr.write_record(&["segment_id"])?;
        for segment_id in &routing_road_segments {
            segment_wtr.write_record(&[segment_id])?;
        }
        segment_wtr.flush()?;
        println!("Optimized tree segment IDs saved successfully.");
    }

    Ok(())
}
