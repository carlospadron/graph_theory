import duckdb

# Bounding box for Oxford
BBOX = {"ymin": 51.5835, "ymax": 51.8528, "xmin": -1.4973, "xmax": -0.9893}

SOURCES = {
    "segments": "s3://overturemaps-us-west-2/release/2026-05-20.0/theme=transportation/type=segment/*",
    "connectors": "s3://overturemaps-us-west-2/release/2026-05-20.0/theme=transportation/type=connector/*",
    "buildings": "s3://overturemaps-us-west-2/release/2026-06-17.0/theme=buildings/type=building/*",
}

_SIMPLE_TYPES = {
    "VARCHAR", "TEXT", "INTEGER", "BIGINT", "HUGEINT", "SMALLINT", "TINYINT",
    "DOUBLE", "FLOAT", "REAL", "BOOLEAN", "DATE", "TIMESTAMP", "BLOB",
}


def _setup_conn() -> duckdb.DuckDBPyConnection:
    conn = duckdb.connect()
    for ext in ("spatial", "httpfs"):
        conn.execute(f"INSTALL {ext}")
        conn.execute(f"LOAD {ext}")
    conn.execute("SET s3_region='us-west-2'")
    return conn


def _gpkg_select(conn: duckdb.DuckDBPyConnection, table: str) -> str:
    """Return a SELECT that casts complex columns (struct/array/map) to VARCHAR."""
    rows = conn.execute(f"DESCRIBE {table}").fetchall()
    cols = []
    for col_name, col_type, *_ in rows:
        upper = col_type.upper()
        is_geometry = upper.startswith("GEOMETRY")
        is_simple = any(upper == t or upper.startswith(t + "(") for t in _SIMPLE_TYPES)
        if is_geometry or is_simple:
            cols.append(f'"{col_name}"')
        else:
            cols.append(f'CAST("{col_name}" AS VARCHAR) AS "{col_name}"')
    return f"SELECT {', '.join(cols)} FROM {table}"


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
        f"COPY ({_gpkg_select(conn, 'data')}) TO '{gpkg_path}' WITH (FORMAT GDAL, DRIVER 'GPKG');"
    )

    print("Data exported successfully to:")
    print(f"  - {parquet_path} (GeoParquet)")
    print(f"  - {gpkg_path} (GeoPackage)")


def extract_routes() -> None:
    extract("segments", SOURCES["segments"])


def extract_connectors() -> None:
    extract("connectors", SOURCES["connectors"])


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
        f"COPY ({_gpkg_select(conn, 'data')}) TO '{gpkg_path}' WITH (FORMAT GDAL, DRIVER 'GPKG');"
    )

    print("Data exported successfully to:")
    print(f"  - {parquet_path} (GeoParquet)")
    print(f"  - {gpkg_path} (GeoPackage)")


def building_centroid_to_nearest_connector() -> None:
    import geopandas as gpd
    from shapely.geometry import LineString

    print("Reading building centroids and connectors from parquet...")
    centroids = gpd.read_parquet("data/oxford_building_centroids.parquet")
    connectors = gpd.read_parquet("data/oxford_connectors.parquet")

    # Use a projected CRS so nearest distance is computed in metres.
    centroids = centroids.to_crs("EPSG:3857")
    connectors = connectors.to_crs("EPSG:3857")

    print(f"  {len(centroids):,} centroids  |  {len(connectors):,} connectors")
    print("Running sjoin_nearest...")

    nearest = (
        centroids[["id", "geometry"]]
        .sjoin_nearest(
            connectors[["id", "geometry"]],
            how="left",
            distance_col="distance_m",
        )
        .rename(columns={"id_left": "building_id", "id_right": "connector_id"})
        .drop_duplicates(subset="building_id")
        .reset_index(drop=True)
    )

    connector_points = connectors[["id", "geometry"]].rename(
        columns={"id": "connector_id", "geometry": "connector_geometry"}
    )
    nearest = nearest.merge(connector_points, on="connector_id", how="left")
    nearest["geometry"] = nearest.apply(
        lambda row: LineString([row["geometry"], row["connector_geometry"]]),
        axis=1,
    )

    line_result = gpd.GeoDataFrame(
        nearest[["building_id", "connector_id", "distance_m", "geometry"]],
        geometry="geometry",
        crs=centroids.crs,
    )

    parquet_path = "data/building_to_connector_lines.parquet"
    gpkg_path = "data/building_to_connector_lines.gpkg"
    line_result.to_parquet(parquet_path, index=False)
    line_result.to_file(gpkg_path, driver="GPKG")
    print(f"Saved {len(line_result):,} rows to {parquet_path}")
    print(f"Saved {len(line_result):,} rows to {gpkg_path}")


def prepare_rust_graph() -> None:
    """Export simplified CSV files that the Rust graph builder consumes."""
    import geopandas as gpd

    conn = _setup_conn()

    print("Exporting connector positions (id, x, y)...")
    conn.execute(
        """
        COPY (
            SELECT id, ST_X(geometry) AS x, ST_Y(geometry) AS y
            FROM read_parquet('data/oxford_connectors.parquet')
        ) TO 'data/rust_connectors.csv' (HEADER, DELIMITER ',')
        """
    )

    print("Exporting building positions (id, x, y)...")
    conn.execute(
        """
        COPY (
            SELECT id, ST_X(geometry) AS x, ST_Y(geometry) AS y
            FROM read_parquet('data/oxford_building_centroids.parquet')
        ) TO 'data/rust_buildings.csv' (HEADER, DELIMITER ',')
        """
    )

    print("Exporting road connector references (road_id, connector_id, at)...")
    conn.execute(
        """
        COPY (
            SELECT
                s.id AS road_id,
                UNNEST(s.connectors).connector_id AS connector_id,
                UNNEST(s.connectors)."at" AS at
            FROM read_parquet('data/oxford_segments.parquet') s
        ) TO 'data/rust_road_connector_refs.csv' (HEADER, DELIMITER ',')
        """
    )

    print("Exporting building links (building_id, connector_id, distance_m)...")
    links = gpd.read_parquet("data/building_to_connector_lines.parquet")
    links[["building_id", "connector_id", "distance_m"]].to_csv(
        "data/rust_building_links.csv", index=False
    )

    print("CSV files ready for Rust graph builder:")
    print("  data/rust_connectors.csv")
    print("  data/rust_buildings.csv")
    print("  data/rust_road_connector_refs.csv")
    print("  data/rust_building_links.csv")


if __name__ == "__main__":
    extract_routes()