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
cargo run --release
```

Reads the four CSVs and builds the undirected multi-graph. Edge weights between consecutive connectors are computed using the Haversine formula from actual connector positions, not from geometry length approximations.

## 5. Building Clustering & Dimension Reduction (Coarsening)

To make optimization over 167,000+ buildings computationally tractable, we implement a macro-level **dimension reduction** framework using graph coarsening:

1. **Deterministic Building Clustering:** 
   We group adjacent buildings using a deterministic **single-linkage clustering** approach along the road network:
   - Evaluates connected components with a Disjoint-Set (Union-Find) data structure.
   - Two buildings belong to the same cluster if they are connected by a path on the road network strictly $\leq 100$ meters.
   - Building-to-connector access distances are treated as $0.0$ meters to ensure linkage is strictly evaluated along the physical roads.
   - Generates deterministic, bounded clusters mapping out dense urban blocks, saving them to `data/rust_clusters.csv`.
   
   To visualize these clusters in QGIS, run:
   - `uv run cluster-buildings` — Merges the cluster mappings back with original building geometries to output `data/clustered_buildings.gpkg`.

2. **Graph Coarsening (Wavefront Propagation):**
   The 255k-node road network is compressed into a sparse, cluster-level **Reduced Graph**:
   - Runs a multi-source Dijkstra / wavefront propagation simultaneously from all cluster boundaries to identify exact shortest road distances between adjacent clusters.
   - Eliminates redundant intermediate street intersections, leaving a high-fidelity coarsened graph where nodes are clusters and edges are shortest boundary-to-boundary connection weights.
   
   To export this reduced topology for visualization, run:
   - `uv run create-reduced-gpkg` — Converts the coarsened node and boundary edges into `data/reduced_clusters.gpkg`.

---

# Optimization

The goal is to identify Pareto-optimal trade-offs between two conflicting objectives:
1. **Maximizing building coverage** (total yield of buildings reached).
2. **Minimizing connection cost** (the Steiner Tree weight of road segments required to connect selected clusters).

## Coarse-to-Fine Optimization Pipeline

With the coarsened `ReducedGraph`, the optimization pipeline natively solves the exact topology:

1. **Super-Node Selection:** The optimizer sorts all 6,614 clusters by size and targets candidates (such as the top 15 largest clusters).
2. **Native Reduced Optimization:** It runs a greedy Pareto search directly on the `ReducedGraph`. The Steiner tree weight is computed using boundary-to-boundary metrics, executing in milliseconds:
   - `optimizer::greedy_reduced_graph_pareto` — Solves cluster selections and produces a sequence of Pareto solutions.
3. **Exact Path Reconstruction:** For the final selected solution, the binary traces back down to the micro-level road network. It executes a Dijkstra path-tracing sweep on the full 255k-node graph to collect the precise, unique Overture road segment IDs traversed to connect all selected hubs, saving them to `data/optimized_tree_segments.csv`.

## Output Generation & Spatial Visualization

Translate the optimized graph selections back into spatial GIS-ready layers:
- `uv run create-optimized-outputs` — Reads the selected building IDs and active road segment IDs, fetches original geometries, and generates:
  - **`data/optimized_selected_buildings.gpkg`** — Original footprints of all buildings in the selected clusters.
  - **`data/optimized_routing_tree.gpkg`** — High-fidelity curved original road path geometries tracing out the exact Steiner routing tree.

Drag these GeoPackages directly into QGIS for instantaneous styling and visual validation!

## Algorithms (Reference)

The Rust optimizer module (`rust/src/optimizer.rs`) implements several core graph algorithms:

| Algorithm | Approach | Complexity | Use case |
|-----------|----------|-----------|----------|
| `brute_force` | Enumerate all $2^n$ subsets, compute exact Steiner tree for each | $O(2^n \cdot n^2 \cdot V \log V)$ | Small candidate sets ($n \le 20$); exact baseline |
| `greedy_pareto` | Iterative cheapest-ratio expansion on the full graph | $O(n^3 \cdot V \log V)$ | Intermediate candidate sets on the micro-graph |
| `greedy_reduced_graph_pareto` | Native greedy Pareto search on the Coarsened Reduced Graph | $O(n^3)$ | Extremely fast macro-level routing over thousands of clusters |
| `steiner_weight` / `reduced_steiner_weight` | Metric closure + Prim's MST (2-approximation for Steiner Tree) | $O(n \cdot V \log V)$ | Core routing metric evaluator |

# Notes
- Bounding box and Overture release URLs are configured in `src/graph_theory/extract_overture_data.py`.
- The Rust crate is located in `rust/` and depends on `petgraph` and `csv`.
- GeoPackage exports natively resolve and reproject `OGC:CRS84` coordinate types.
