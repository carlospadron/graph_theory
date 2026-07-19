import argparse
import pandas as pd
import geopandas as gpd
from shapely.geometry import Point

def main():
    parser = argparse.ArgumentParser(description="Create GeoPackage for reduced cluster nodes")
    parser.add_argument("--nodes", default="data/reduced_nodes.csv", help="Path to reduced nodes CSV")
    parser.add_argument("--output", default="data/reduced_clusters.gpkg", help="Path to output GPKG")
    args = parser.parse_args()

    print(f"Reading reduced nodes from {args.nodes}...")
    df = pd.read_csv(args.nodes)

    print("Converting to geometry points...")
    geometry = [Point(xy) for xy in zip(df["centroid_x"], df["centroid_y"])]
    
    # Create GeoDataFrame
    # Assuming WGS-84 (EPSG:4326) coordinates for centroid positions in Rust output
    gdf = gpd.GeoDataFrame(df, geometry=geometry, crs="EPSG:4326")

    print(f"Saving to {args.output}...")
    gdf.to_file(args.output, driver="GPKG")
    print("Done!")

if __name__ == "__main__":
    main()
