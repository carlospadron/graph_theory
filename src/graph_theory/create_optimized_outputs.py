import argparse
import pandas as pd
import geopandas as gpd
from shapely.geometry import LineString

def main():
    parser = argparse.ArgumentParser(description="Create GeoPackages for optimized solution outputs")
    parser.add_argument("--buildings-gpkg", default="data/oxford_buildings.gpkg", help="Path to original buildings GPKG")
    parser.add_argument("--selected-csv", default="data/selected_buildings.csv", help="Path to selected buildings CSV")
    parser.add_argument("--segments-csv", default="data/optimized_tree_segments.csv", help="Path to optimized tree segments CSV")
    parser.add_argument("--segments-parquet", default="data/oxford_segments.parquet", help="Path to original segments Parquet")
    parser.add_argument("--out-buildings", default="data/optimized_selected_buildings.gpkg", help="Output GPKG for selected buildings")
    parser.add_argument("--out-tree", default="data/optimized_routing_tree.gpkg", help="Output GPKG for optimized road tree")
    args = parser.parse_args()

    print(f"Reading original buildings from {args.buildings_gpkg}...")
    buildings = gpd.read_file(args.buildings_gpkg)
    # Extract the exact CRS definition from the source data to match perfectly
    target_crs = buildings.crs

    print(f"Reading selected building IDs from {args.selected_csv}...")
    selected_df = pd.read_csv(args.selected_csv)

    print("Filtering selected buildings...")
    selected_buildings = buildings.merge(selected_df, left_on="id", right_on="building_id", how="inner")

    print(f"Saving selected buildings to {args.out_buildings}...")
    selected_buildings.to_file(args.out_buildings, driver="GPKG")

    print(f"Reading optimized tree segment IDs from {args.segments_csv}...")
    segments_df = pd.read_csv(args.segments_csv)

    print(f"Reading original segments from {args.segments_parquet}...")
    segments_gdf = gpd.read_parquet(args.segments_parquet)

    print("Filtering selected road segments...")
    # Merge or filter segments based on active segment IDs
    tree_gdf = segments_gdf.merge(segments_df, left_on="id", right_on="segment_id", how="inner")

    # Ensure the CRS is strictly matched to target_crs
    if tree_gdf.crs != target_crs:
        print(f"Reprojecting road segments from {tree_gdf.crs} to {target_crs}...")
        tree_gdf = tree_gdf.to_crs(target_crs)
    print(f"Saving optimized routing tree to {args.out_tree}...")
    tree_gdf.to_file(args.out_tree, driver="GPKG")

    print("\nOutputs generated successfully!")
    print(f"  - Selected Buildings GeoPackage:  {args.out_buildings}")
    print(f"  - Optimized Routing Tree GeoPackage: {args.out_tree}")

if __name__ == "__main__":
    main()

