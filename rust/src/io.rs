use crate::{Building, BuildingConnectorLink, Connector, RoadConnectorRef, RoadSegment};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

/// Haversine distance in metres between two WGS-84 lon/lat points.
pub fn haversine_m(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    const R: f64 = 6_371_000.0;
    let phi1 = lat1.to_radians();
    let phi2 = lat2.to_radians();
    let dphi = (lat2 - lat1).to_radians();
    let dlambda = (lon2 - lon1).to_radians();
    let a = (dphi / 2.0).sin().powi(2)
        + phi1.cos() * phi2.cos() * (dlambda / 2.0).sin().powi(2);
    2.0 * R * a.sqrt().asin()
}

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
/// (columns: road_id, connector_id, at) and group them into `RoadSegment`s.
///
/// Edge lengths are computed as the Haversine distance between each pair of
/// adjacent connectors, so `connector_positions` (lon, lat per connector id)
/// must be provided.
pub fn read_roads(
    path: impl AsRef<Path>,
    connector_positions: &HashMap<String, (f64, f64)>,
) -> Result<Vec<RoadSegment>, Box<dyn Error>> {
    let mut rdr = csv::Reader::from_path(path)?;

    let mut road_map: HashMap<String, Vec<RoadConnectorRef>> = HashMap::new();
    for result in rdr.records() {
        let r = result?;
        road_map
            .entry(r[0].to_string())
            .or_default()
            .push(RoadConnectorRef {
                connector_id: r[1].to_string(),
                at: r[2].parse()?,
            });
    }

    let mut roads = Vec::with_capacity(road_map.len());
    for (road_id, mut refs) in road_map {
        refs.sort_by(|a, b| a.at.total_cmp(&b.at));

        let length_m: f64 = refs
            .windows(2)
            .map(|pair| {
                match (
                    connector_positions.get(&pair[0].connector_id),
                    connector_positions.get(&pair[1].connector_id),
                ) {
                    (Some(&(x1, y1)), Some(&(x2, y2))) => haversine_m(x1, y1, x2, y2),
                    _ => 0.0,
                }
            })
            .sum();

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
