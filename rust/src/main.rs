use graph_builder::io;
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

    println!("\nBuilding directed graph...");
    let directed = graph_builder::build_directed_graph(&roads, &connectors, &buildings, &links)?;
    println!(
        "  nodes: {}   edges: {}",
        directed.graph.node_count(),
        directed.graph.edge_count()
    );

    Ok(())
}
