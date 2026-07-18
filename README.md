# graph_theory
Research and Development on graph optimization, comparing reinforcement learning approaches against bespoke algorithms.

The study area is Oxford, UK. Overture Maps transportation and building data are used to construct a spatial graph where connectors are nodes, road segments are edges, and buildings are leaf nodes attached to their nearest connector. The graph is implemented in Rust using `petgraph` and is available in both undirected and directed (bidirectional) forms.

# Pipeline

Run the steps below in order. Use `uv run <command>` for all Python steps.

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
cargo run --release --bin build_graph [OPTION]
```

Reads the four CSVs and builds two graphs:
- **Undirected** (`build_graph`) — for standard graph optimization.
- **Directed bidirectional** (`build_directed_graph`) — all edges added in both directions; ready for one-way street modelling once Overture access restrictions are applied.

Edge weights between consecutive connectors are computed using the Haversine formula from actual connector positions, not from geometry length approximations.

### Candidate selection (command-line options)

Pass an option to specify which nodes to optimize over. If none is provided, defaults to the first 12 connectors with brute-force.

- `--targets-file PATH` — Read node IDs from a file (one per line; IDs can be connector or building IDs from the CSVs)
- `--all-buildings` — Optimize over all buildings (greedy Pareto)
- `--all-connectors` — Optimize over all connectors (greedy Pareto)  
- `--first-n-connectors N` — Optimize over first N connectors (brute if N ≤ 20, else greedy)
- `--first-n-buildings N` — Optimize over first N buildings (brute if N ≤ 20, else greedy)
- `--help` — Show usage

**Examples:**
```bash
# Select specific nodes from a file
cargo run --release --bin build_graph -- --targets-file targets.txt

# Select first 8 connectors
cargo run --release --bin build_graph -- --first-n-connectors 8

# Optimize all buildings
cargo run --release --bin build_graph -- --all-buildings
```

The targets file format is simple — one node ID per line:
```
connector_abc123
building_xyz789
connector_def456
```

# Optimization

The goal is to select a subset of graph nodes (buildings/connectors) such that:
- The number of selected nodes is maximized.
- The cost of connecting them via the road network (Steiner tree weight) is minimized.

These objectives conflict, so the solution is a **Pareto front** — a set of non-dominated trade-off points.

## Algorithms

The Rust optimizer module (`rust/src/optimizer.rs`) implements three approaches:

| Algorithm | Approach | Complexity | Use case |
|-----------|----------|-----------|----------|
| `brute_force` | Enumerate all 2^n subsets, compute Steiner tree for each | O(2^n · n · V log V) | n ≤ ~20 candidates; optimal Pareto front |
| `greedy_pareto` | Iteratively add the cheapest candidate | O(n² · V log V) | Large candidate sets (100s); greedy approximation |
| `steiner_weight` | Metric closure + Prim's MST (2-approximation for Steiner tree) | O(n · V log V) per call | Core function; all-pairs Dijkstra + MST |

The binary selects brute-force for n ≤ 20 and greedy for larger sets.

# Notes
- Bounding box and Overture release URLs are configured in `src/graph_theory/extract_overture_data.py`.
- The Rust crate is in `rust/` and depends on `petgraph` and `csv`.
- GeoPackage exports cast complex DuckDB types (struct arrays) to VARCHAR so GDAL can write them.


## 5. Building Clustering

We implemented a **single-linkage clustering** approach on the road graph network to group adjacent buildings:
- `cluster_buildings` (in Rust) — Finds exact connected components. Two buildings belong to the same cluster if they are connected by a path on the road network strictly $\leq$ 100 meters. (Building access distance is ignored, meaning distance is strictly evaluated on the actual road network).
- This creates deterministically chained clusters mapping out dense urban blocks.
- Outputs the grouping to `data/rust_clusters.csv`.

**Merge Clusters with Geometry:**
- `uv run cluster-buildings` — Merges the `data/rust_clusters.csv` mapping back into `data/oxford_buildings.gpkg` and outputs a standalone file `data/clustered_buildings.gpkg` that you can directly load into QGIS for spatial visualization.
