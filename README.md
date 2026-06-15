# graph_theory
Research and Development on graph theory issues.


# Data
- Run `python scripts/extract_routes.py` to download road segment data from Overture Maps.
- Command example: `uv run extract-routes`
- The script queries Overture transportation segments and writes the result to `data/rome_segments.parquet`.


# TODO
- data: os open roads, os open uprn. Preferably a dataset ready for routing
- build spatial graph
- dbscan clustering
- optimisation with rust written algorithms
- optimisation with reinforcement learning 
