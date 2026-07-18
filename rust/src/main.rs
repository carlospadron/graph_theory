use graph_builder::{io, clustering, optimizer};
use std::collections::HashMap;
use std::path::Path;
use petgraph::graph::NodeIndex;

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

    // Group into clusters_map for the optimizer
    let mut clusters_map: HashMap<usize, Vec<NodeIndex>> = HashMap::new();
    for (&node_idx, &cluster_id) in &clusters {
        clusters_map.entry(cluster_id).or_default().push(node_idx);
    }

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

    // Run cluster-based optimizer
    println!("\nRunning Cluster-Level Greedy Pareto Search...");
    
    // The metric closure (all-pairs shortest path via Dijkstra) is O(N^2 * (V+E) log V).
    // Let's run it on just 10 clusters to keep execution time under 10 seconds for the demo.
    let mut sorted_clusters: Vec<_> = clusters_map.iter().collect();
    sorted_clusters.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    let top_clusters: HashMap<usize, Vec<NodeIndex>> = sorted_clusters.into_iter().take(10).map(|(&k, v)| (k, v.clone())).collect();

    println!("(Evaluating on top {} largest clusters to quickly demonstrate)...", top_clusters.len());
    let solutions = optimizer::greedy_cluster_pareto(&built, &top_clusters);

    if !solutions.is_empty() {
        println!("  Step  1: {} cluster(s) selected, {} buildings yielded, {:.1} m tree weight", 
            solutions[0].selected_clusters.len(), solutions[0].total_buildings_yield, solutions[0].tree_weight);
        
        if solutions.len() > 1 {
            let mid = solutions.len() / 2;
            println!("  Step {:>2}: {} cluster(s) selected, {} buildings yielded, {:.1} m tree weight", 
                mid + 1, solutions[mid].selected_clusters.len(), solutions[mid].total_buildings_yield, solutions[mid].tree_weight);
        }
        let last = solutions.last().unwrap();
        println!("  Step {:>2}: {} cluster(s) selected, {} buildings yielded, {:.1} m tree weight", 
            solutions.len(), last.selected_clusters.len(), last.total_buildings_yield, last.tree_weight);
    }

    Ok(())
}
