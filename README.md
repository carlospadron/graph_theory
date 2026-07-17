# graph_theory
Research and Development on graph optimization, comparing reinforcement learning approaches against bespoke algorithms.

The study area is Oxford, UK. Overture Maps transportation and building data are used to construct a spatial graph where connectors are nodes, road segments are edges, and buildings are leaf nodes attached to their nearest connector. The graph is implemented in Rust using `petgraph` and is available in both undirected and directed (bidirectional) forms.

# Pipeline

Run the steps below in order. All Python scripts are available via `uv run <command>` or `python scripts/<script>.py`.

## 1. Download data from Overture Maps

- `uv run extract-routes` — road segments → `data/oxford_segments.parquet` / `.gpkg`
- `uv run extract-connectors` — connector nodes → `data/oxford_connectors.parquet` / `.gpkg`
- `uv run extract-buildings` — building footprints → `data/oxford_buildings.parquet` / `.gpkg`

## 2. Derive spatial features

- `uv run extract-building-centroids` — centroid per building → `data/oxford_building_centroids.parquet` / `.gpkg`
- `uv run building-to-connector` — nearest connector per centroid (sjoin_nearest, EPSG:3857) → `data/building_to_connector_lines.parquet` / `.gpkg`

## 3. Prepare CSV inputs for the Rust graph builder

- `uv run prepare-rust-graph` — flattens the parquet files into four CSVs the Rust crate reads directly:
  - `data/rust_connectors.csv` (id, x, y)
  - `data/rust_buildings.csv` (id, x, y)
  - `data/rust_road_connector_refs.csv` (road_id, connector_id, at)
  - `data/rust_building_links.csv` (building_id, connector_id, distance_m)

## 4. Build the graph in Rust

```bash
cd rust
cargo run --release --bin build_graph
```

Reads the four CSVs and builds two graphs:
- **Undirected** (`build_graph`) — for standard graph optimization.
- **Directed bidirectional** (`build_directed_graph`) — all edges added in both directions; ready for one-way street modelling once Overture access restrictions are applied.

Edge weights between consecutive connectors are computed using the Haversine formula from actual connector positions, not from geometry length approximations.

# Notes
- Bounding box and Overture release URLs are configured in `src/graph_theory/extract_overture_data.py`.
- The Rust crate is in `rust/` and depends on `petgraph` and `csv`.
- GeoPackage exports cast complex DuckDB types (struct arrays) to VARCHAR so GDAL can write them.

