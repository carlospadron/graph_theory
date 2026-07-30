use graph_builder::{io, clustering, reduced_graph, optimizer};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::{BufRead, BufReader};

#[derive(Debug)]
struct CliOptions {
    sources_file: Option<String>,
    source_ids: Vec<String>,
}

fn print_usage() {
    eprintln!("Usage: build_graph [--sources-file PATH] [--source-id NODE_ID ...]");
    eprintln!("  --sources-file PATH   File with source node IDs (one per line)");
    eprintln!("  --source-id NODE_ID   Source node ID (repeatable)");
    eprintln!("  --help                Show this help");
}

fn parse_cli() -> Result<CliOptions, String> {
    let mut sources_file: Option<String> = None;
    let mut source_ids: Vec<String> = Vec::new();

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            "--sources-file" => {
                let val = args
                    .next()
                    .ok_or_else(|| "missing value for --sources-file".to_string())?;
                sources_file = Some(val);
            }
            "--source-id" => {
                let val = args
                    .next()
                    .ok_or_else(|| "missing value for --source-id".to_string())?;
                source_ids.push(val);
            }
            _ => {
                return Err(format!("unknown argument: {arg}"));
            }
        }
    }

    if sources_file.is_none() && source_ids.is_empty() {
        return Err("at least one source must be provided via --sources-file or --source-id".to_string());
    }

    Ok(CliOptions {
        sources_file,
        source_ids,
    })
}

fn load_source_ids(opts: &CliOptions) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ids = opts.source_ids.clone();

    if let Some(path) = &opts.sources_file {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            ids.push(trimmed.to_string());
        }
    }

    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));

    if ids.is_empty() {
        return Err("source list is empty after parsing inputs".into());
    }

    Ok(ids)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opts = parse_cli().map_err(|e| {
        print_usage();
        e
    })?;
    let source_ids = load_source_ids(&opts)?;
    println!("Source constraint: {} input node IDs", source_ids.len());

    let data = Path::new("../data");

    let connectors = io::read_connectors(data.join("rust_connectors.csv"))?;
    println!("Connectors loaded:      {:>7}", connectors.len());

    let buildings = io::read_buildings(data.join("rust_buildings.csv"))?;
    println!("Buildings loaded:       {:>7}", buildings.len());

    let roads = io::read_roads(data.join("rust_road_connector_refs.csv"))?;
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
    let (r_graph, nearest_cluster_by_node) = reduced_graph::build_reduced_graph(&built, &clusters);
    println!(
        "Reduced Graph created!\n  nodes (clusters): {}   edges (boundary connections): {}",
        r_graph.node_count(),
        r_graph.edge_count()
    );

    let mut source_cluster_ids: HashSet<usize> = HashSet::new();
    let mut resolved_sources = 0usize;
    let mut unresolved_sources: Vec<String> = Vec::new();

    for source_id in &source_ids {
        let source_node = built
            .building_nodes
            .get(source_id)
            .copied()
            .or_else(|| built.connector_nodes.get(source_id).copied());

        if let Some(node_idx) = source_node {
            resolved_sources += 1;
            if let Some(&cluster_id) = clusters.get(&node_idx) {
                source_cluster_ids.insert(cluster_id);
            } else if let Some(&cluster_id) = nearest_cluster_by_node.get(&node_idx) {
                source_cluster_ids.insert(cluster_id);
            }
        } else {
            unresolved_sources.push(source_id.clone());
        }
    }

    if resolved_sources == 0 {
        return Err("none of the provided source IDs match graph nodes".into());
    }
    if source_cluster_ids.is_empty() {
        return Err("source IDs resolved, but none could be mapped to clusters".into());
    }
    if !unresolved_sources.is_empty() {
        println!("Warning: {} source IDs were not found in graph data.", unresolved_sources.len());
    }
    println!(
        "Source constraint resolved: {} matched nodes -> {} source clusters",
        resolved_sources,
        source_cluster_ids.len()
    );

    let mut cluster_id_to_rnode: HashMap<usize, NodeIndex> = HashMap::new();
    for idx in r_graph.node_indices() {
        cluster_id_to_rnode.insert(r_graph[idx].cluster_id, idx);
    }

    let mut source_reduced_nodes: Vec<NodeIndex> = Vec::new();
    for source_cluster_id in &source_cluster_ids {
        if let Some(&rnode) = cluster_id_to_rnode.get(source_cluster_id) {
            source_reduced_nodes.push(rnode);
        }
    }
    if source_reduced_nodes.is_empty() {
        return Err("source clusters could not be mapped to reduced graph nodes".into());
    }

    // Run optimizer directly on the Reduced Graph
    println!("\nRunning budgeted source-constrained optimizer on the Reduced Graph...");
    let budget_m_per_building = 300.0;
    let epsilon = 1.5;
    let iterations = 100usize;
    println!(
        "(Budget search: budget={:.1} m/building, epsilon={:.2}, iterations={})...",
        budget_m_per_building,
        epsilon,
        iterations
    );
    let chosen = optimizer::budgeted_source_search(
        &r_graph,
        &source_reduced_nodes,
        budget_m_per_building,
        epsilon,
        iterations,
    )
    .ok_or("budgeted optimizer could not produce a solution")?;

        println!("  Optimized Solution: {} clusters selected, {} buildings yielded, {:.1} m tree weight", 
            chosen.selected_cluster_ids.len(), chosen.total_buildings_yield, chosen.tree_weight);

        // Save selected building IDs
        println!("\nSaving selected building IDs to data/selected_buildings.csv...");
        let mut target_wtr = csv::Writer::from_path(data.join("selected_buildings.csv"))?;
        target_wtr.write_record(&["building_id", "cluster_id"])?;

        let mut selected_cluster_ids = std::collections::HashSet::new();
        for &cid in &chosen.selected_cluster_ids {
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
        for &cid in &chosen.selected_cluster_ids {
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

    Ok(())
}
