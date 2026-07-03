# graph_theory
Research and Development on graph theory issues.


# Data
- `python scripts/extract_routes.py` or `uv run extract-routes`
	- Download Oxford road segments from Overture Maps.
	- Writes `data/oxford_segments.parquet` and `data/oxford_segments.gpkg`.
- `python scripts/extract_connectors.py` or `uv run extract-connectors`
	- Download Oxford connectors from Overture Maps.
	- Writes `data/oxford_connectors.parquet` and `data/oxford_connectors.gpkg`.
- `python scripts/extract_buildings.py` or `uv run extract-buildings`
	- Download Oxford buildings from Overture Maps.
	- Writes `data/oxford_buildings.parquet` and `data/oxford_buildings.gpkg`.
- `python scripts/extract_building_centroids.py` or `uv run extract-building-centroids`
	- Build centroid points for Oxford buildings.
	- Writes `data/oxford_building_centroids.parquet` and `data/oxford_building_centroids.gpkg`.
- `python scripts/building_to_connector.py` or `uv run building-to-connector`
	- Find the nearest connector for each building centroid.
	- Writes `data/building_to_connector_lines.parquet` and `data/building_to_connector_lines.gpkg` with line geometries.

# Notes
- All scripts use the same Oxford bounding box and Overture Maps release window configured in `src/graph_theory/extract_overture_data.py`.



