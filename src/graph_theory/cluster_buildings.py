import argparse
import duckdb
import geopandas as gpd

def run(clusters_path="data/rust_clusters.csv", buildings_path="data/oxford_buildings.gpkg", output_path="data/clustered_buildings.gpkg"):
    print(f"Reading buildings from {buildings_path}...")
    buildings = gpd.read_file(buildings_path)
    
    print(f"Reading clusters from {clusters_path}...")
    conn = duckdb.connect()
    clusters = conn.read_csv(clusters_path).df()

    print("Merging...")
    merged = buildings.merge(clusters, left_on='id', right_on='building_id', how='left')

    print(f"Saving to {output_path}...")
    merged.to_file(output_path, driver='GPKG')
    print("Done!")

def main():
    parser = argparse.ArgumentParser(description="Merge rust clusters with buildings GPKG")
    parser.add_argument("--clusters", default="data/rust_clusters.csv", help="Path to clusters CSV")
    parser.add_argument("--buildings", default="data/oxford_buildings.gpkg", help="Path to buildings GPKG")
    parser.add_argument("--output", default="data/clustered_buildings.gpkg", help="Path to output GPKG")
    args = parser.parse_args()
    
    run(args.clusters, args.buildings, args.output)

if __name__ == '__main__':
    main()
