use crate::{Building, BuildingConnectorLink, Connector, RoadConnectorRef, RoadSegment};
use std::error::Error;
use std::path::Path;

/// Read connectors from `rust_connectors.csv` (columns: id, x, y).
pub fn read_connectors(path: impl AsRef<Path>) -> Result<Vec<Connector>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut out = Vec::new();
    for result in rdr.records() {
        let r = result?;
        out.push(Connector {
            id: r[0].to_string(),
            x: r[1].parse()?,
            y: r[2].parse()?,
        });
    }
    Ok(out)
}

/// Read buildings from `rust_buildings.csv` (columns: id, x, y).
pub fn read_buildings(path: impl AsRef<Path>) -> Result<Vec<Building>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut out = Vec::new();
    for result in rdr.records() {
        let r = result?;
        out.push(Building {
            id: r[0].to_string(),
            x: r[1].parse()?,
            y: r[2].parse()?,
        });
    }
    Ok(out)
}

/// Read road connector references from `rust_road_connector_refs.csv`
/// (columns: road_id, connector_id, at, road_length_m) and group them into
/// `RoadSegment`s.
pub fn read_roads(path: impl AsRef<Path>) -> Result<Vec<RoadSegment>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;

    let mut road_map: std::collections::HashMap<String, (f64, Vec<RoadConnectorRef>)> =
        std::collections::HashMap::new();
    for result in rdr.records() {
        let r = result?;
        if r.len() < 4 {
            return Err("rust_road_connector_refs.csv must contain columns: road_id,connector_id,at,road_length_m".into());
        }

        let road_id = r[0].to_string();
        let road_length_m: f64 = r[3].parse()?;

        let entry = road_map
            .entry(road_id)
            .or_insert_with(|| (road_length_m, Vec::new()));

        entry.1.push(RoadConnectorRef {
            connector_id: r[1].to_string(),
            at: r[2].parse()?,
        });

        if (entry.0 - road_length_m).abs() > 1e-6 {
            entry.0 = road_length_m;
        }
    }

    let mut roads = Vec::with_capacity(road_map.len());
    for (road_id, (length_m, mut refs)) in road_map {
        refs.sort_by(|a, b| a.at.total_cmp(&b.at));

        roads.push(RoadSegment {
            id: road_id,
            length_m,
            connectors: refs,
        });
    }

    Ok(roads)
}

/// Read building-to-connector links from `rust_building_links.csv`
/// (columns: building_id, connector_id, distance_m).
pub fn read_building_links(
    path: impl AsRef<Path>,
) -> Result<Vec<BuildingConnectorLink>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;
    let mut out = Vec::new();
    for result in rdr.records() {
        let r = result?;
        out.push(BuildingConnectorLink {
            building_id: r[0].to_string(),
            connector_id: r[1].to_string(),
            distance_m: r[2].parse()?,
        });
    }
    Ok(out)
}
