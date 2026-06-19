import duckdb

# Bounding box for Oxford
BBOX = {"ymin": 51.5835, "ymax": 51.8528, "xmin": -1.4973, "xmax": -0.9893}

SOURCES = {
    "segments": "s3://overturemaps-us-west-2/release/2026-05-20.0/theme=transportation/type=segment/*",
    "buildings": "s3://overturemaps-us-west-2/release/2026-06-17.0/theme=buildings/type=building/*",
}


def _setup_conn() -> duckdb.DuckDBPyConnection:
    conn = duckdb.connect()
    for ext in ("spatial", "httpfs"):
        conn.execute(f"INSTALL {ext}")
        conn.execute(f"LOAD {ext}")
    conn.execute("SET s3_region='us-west-2'")
    return conn


def extract(name: str, s3_path: str) -> None:
    conn = _setup_conn()

    print(f"Downloading Oxford {name} from Overture Maps...")
    conn.execute(
        f"""
        CREATE TEMP TABLE data AS
        SELECT *
        FROM read_parquet('{s3_path}')
        WHERE
            bbox.ymin > {BBOX['ymin']} AND bbox.ymax < {BBOX['ymax']} AND
            bbox.xmin > {BBOX['xmin']} AND bbox.xmax < {BBOX['xmax']}
        """
    )

    result = conn.execute("SELECT COUNT(*) FROM data").fetchone()
    print(f"Number of Oxford {name} downloaded and filtered: {result[0]}")

    parquet_path = f"data/oxford_{name}.parquet"
    gpkg_path = f"data/oxford_{name}.gpkg"

    print("Exporting to GeoParquet and GeoPackage formats...")
    conn.execute(
        f"COPY data TO '{parquet_path}' (FORMAT PARQUET, COMPRESSION SNAPPY)"
    )
    print("Exported to GeoParquet successfully.")

    print("Exporting to GeoPackage format...")
    conn.execute(
        f"COPY data TO '{gpkg_path}' WITH (FORMAT GDAL, DRIVER 'GPKG');"
    )

    print(f"Data exported successfully to:")
    print(f"  - {parquet_path} (GeoParquet)")
    print(f"  - {gpkg_path} (GeoPackage)")


def extract_routes() -> None:
    extract("segments", SOURCES["segments"])


def extract_buildings() -> None:
    extract("buildings", SOURCES["buildings"])


def extract_building_centroids() -> None:
    conn = _setup_conn()

    print("Extracting Oxford building centroids from Overture Maps...")
    conn.execute(
        f"""
        CREATE TEMP TABLE data AS
        SELECT
            id,
            ST_Centroid(geometry) AS geometry
        FROM read_parquet('{SOURCES["buildings"]}')
        WHERE
            bbox.ymin > {BBOX['ymin']} AND bbox.ymax < {BBOX['ymax']} AND
            bbox.xmin > {BBOX['xmin']} AND bbox.xmax < {BBOX['xmax']}
        """
    )

    result = conn.execute("SELECT COUNT(*) FROM data").fetchone()
    print(f"Number of Oxford building centroids: {result[0]}")

    parquet_path = "data/oxford_building_centroids.parquet"
    gpkg_path = "data/oxford_building_centroids.gpkg"

    print("Exporting to GeoParquet and GeoPackage formats...")
    conn.execute(
        f"COPY data TO '{parquet_path}' (FORMAT PARQUET, COMPRESSION SNAPPY)"
    )
    print("Exported to GeoParquet successfully.")

    print("Exporting to GeoPackage format...")
    conn.execute(
        f"COPY data TO '{gpkg_path}' WITH (FORMAT GDAL, DRIVER 'GPKG');"
    )

    print("Data exported successfully to:")
    print(f"  - {parquet_path} (GeoParquet)")
    print(f"  - {gpkg_path} (GeoPackage)")


if __name__ == "__main__":
    extract_routes()
