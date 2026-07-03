use petgraph::graph::{Graph, NodeIndex};
use petgraph::Undirected;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone)]
pub struct RoadSegment {
    pub id: String,
    pub length_m: f64,
    pub connectors: Vec<RoadConnectorRef>,
}

#[derive(Debug, Clone)]
pub struct RoadConnectorRef {
    pub connector_id: String,
    pub at: f64,
}

#[derive(Debug, Clone)]
pub struct Connector {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct Building {
    pub id: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct BuildingConnectorLink {
    pub building_id: String,
    pub connector_id: String,
    pub distance_m: f64,
}

#[derive(Debug, Clone)]
pub enum Node {
    Connector { id: String, x: f64, y: f64 },
    Building { id: String, x: f64, y: f64 },
}

#[derive(Debug, Clone)]
pub enum Edge {
    Road { segment_id: String, length_m: f64 },
    BuildingAccess { distance_m: f64 },
}

pub type SpatialGraph = Graph<Node, Edge, Undirected>;

#[derive(Debug, Clone)]
pub struct BuiltGraph {
    pub graph: SpatialGraph,
    pub connector_nodes: HashMap<String, NodeIndex>,
    pub building_nodes: HashMap<String, NodeIndex>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildGraphError {
    DuplicateConnector(String),
    DuplicateBuilding(String),
    MissingConnector(String),
    MissingBuilding(String),
    RoadSegmentTooShort(String),
}

impl fmt::Display for BuildGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateConnector(id) => write!(f, "duplicate connector id: {id}"),
            Self::DuplicateBuilding(id) => write!(f, "duplicate building id: {id}"),
            Self::MissingConnector(id) => write!(f, "missing connector id: {id}"),
            Self::MissingBuilding(id) => write!(f, "missing building id: {id}"),
            Self::RoadSegmentTooShort(id) => {
                write!(f, "road segment {id} must reference at least two connectors")
            }
        }
    }
}

impl std::error::Error for BuildGraphError {}

/// Build an undirected graph where:
/// - connector features are graph nodes,
/// - road segments become weighted edges between consecutive connectors,
/// - buildings are leaf nodes attached to their nearest connector.
///
/// `RoadSegment::length_m` is distributed proportionally between consecutive
/// connectors using the connector `at` values.
pub fn build_graph(
    roads: &[RoadSegment],
    connectors: &[Connector],
    buildings: &[Building],
    building_links: &[BuildingConnectorLink],
) -> Result<BuiltGraph, BuildGraphError> {
    let mut graph: SpatialGraph = Graph::new_undirected();
    let mut connector_nodes: HashMap<String, NodeIndex> = HashMap::new();
    let mut building_nodes: HashMap<String, NodeIndex> = HashMap::new();

    for connector in connectors {
        if connector_nodes.contains_key(&connector.id) {
            return Err(BuildGraphError::DuplicateConnector(connector.id.clone()));
        }
        let node = graph.add_node(Node::Connector {
            id: connector.id.clone(),
            x: connector.x,
            y: connector.y,
        });
        connector_nodes.insert(connector.id.clone(), node);
    }

    for building in buildings {
        if building_nodes.contains_key(&building.id) {
            return Err(BuildGraphError::DuplicateBuilding(building.id.clone()));
        }
        let node = graph.add_node(Node::Building {
            id: building.id.clone(),
            x: building.x,
            y: building.y,
        });
        building_nodes.insert(building.id.clone(), node);
    }

    for road in roads {
        if road.connectors.len() < 2 {
            return Err(BuildGraphError::RoadSegmentTooShort(road.id.clone()));
        }

        let mut refs = road.connectors.clone();
        refs.sort_by(|left, right| left.at.total_cmp(&right.at));

        for pair in refs.windows(2) {
            let from = &pair[0];
            let to = &pair[1];

            let from_idx = *connector_nodes
                .get(&from.connector_id)
                .ok_or_else(|| BuildGraphError::MissingConnector(from.connector_id.clone()))?;
            let to_idx = *connector_nodes
                .get(&to.connector_id)
                .ok_or_else(|| BuildGraphError::MissingConnector(to.connector_id.clone()))?;

            let segment_fraction = (to.at - from.at).abs();
            let edge_length = road.length_m * segment_fraction;
            graph.add_edge(
                from_idx,
                to_idx,
                Edge::Road {
                    segment_id: road.id.clone(),
                    length_m: edge_length,
                },
            );
        }
    }

    for link in building_links {
        let building_idx = *building_nodes
            .get(&link.building_id)
            .ok_or_else(|| BuildGraphError::MissingBuilding(link.building_id.clone()))?;
        let connector_idx = *connector_nodes
            .get(&link.connector_id)
            .ok_or_else(|| BuildGraphError::MissingConnector(link.connector_id.clone()))?;

        graph.add_edge(
            building_idx,
            connector_idx,
            Edge::BuildingAccess {
                distance_m: link.distance_m,
            },
        );
    }

    Ok(BuiltGraph {
        graph,
        connector_nodes,
        building_nodes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_small_graph() {
        let roads = vec![RoadSegment {
            id: "r1".to_string(),
            length_m: 100.0,
            connectors: vec![
                RoadConnectorRef {
                    connector_id: "c1".to_string(),
                    at: 0.0,
                },
                RoadConnectorRef {
                    connector_id: "c2".to_string(),
                    at: 1.0,
                },
            ],
        }];

        let connectors = vec![
            Connector {
                id: "c1".to_string(),
                x: 0.0,
                y: 0.0,
            },
            Connector {
                id: "c2".to_string(),
                x: 1.0,
                y: 0.0,
            },
        ];

        let buildings = vec![Building {
            id: "b1".to_string(),
            x: 0.2,
            y: 0.1,
        }];

        let building_links = vec![BuildingConnectorLink {
            building_id: "b1".to_string(),
            connector_id: "c1".to_string(),
            distance_m: 12.5,
        }];

        let built = build_graph(&roads, &connectors, &buildings, &building_links).unwrap();
        assert_eq!(built.graph.node_count(), 3);
        assert_eq!(built.graph.edge_count(), 2);
        assert_eq!(built.connector_nodes.len(), 2);
        assert_eq!(built.building_nodes.len(), 1);
    }
}
