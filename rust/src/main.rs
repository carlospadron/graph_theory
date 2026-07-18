use graph_builder::{io, clustering};
use std::collections::HashMap;
use std::path::Path;

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

    Ok(())
}
