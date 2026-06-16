import duckdb


def main() -> None:
    conn = duckdb.connect()

    conn.execute("INSTALL spatial")
    conn.execute("INSTALL httpfs")
    conn.execute("LOAD spatial")
    conn.execute("LOAD httpfs")

    conn.execute("SET s3_region='us-west-2'")

    print("Downloading Oxford segments from Overture Maps...")
    # Create temporary table from downloaded data
    conn.execute(
        """
        CREATE TEMP TABLE oxford_segments AS
        SELECT
            *
        FROM read_parquet('s3://overturemaps-us-west-2/release/2026-05-20.0/theme=transportation/type=segment/*')
        WHERE
            bbox.ymin > 51.5835 AND bbox.ymax < 51.8528 AND
            bbox.xmin > -1.4973 AND bbox.xmax < -0.9893
        """
    )

    #count the number of rows in the temporary table
    result = conn.execute("SELECT COUNT(*) FROM oxford_segments").fetchone()
    print(f"Number of Oxford segments downloaded and filtered: {result[0]}")

    print("Downloaded and filtered Oxford segments successfully.")
    print("Exporting to GeoParquet and GeoPackage formats...")
    # Export to both formats
    conn.execute(
        """
        COPY oxford_segments
        TO 'data/oxford_segments.parquet'
        (FORMAT PARQUET, COMPRESSION SNAPPY)
        """
    )

    print("Exported to GeoParquet successfully.")
    print("Exporting to GeoPackage format...")
    conn.execute(
        """
        COPY oxford_segments
        TO 'data/oxford_segments.gpkg'
        WITH (FORMAT GDAL, DRIVER 'GPKG');
        """
    )

    print("Data exported successfully to:")
    print("  - data/oxford_segments.parquet (GeoParquet)")
    print("  - data/oxford_segments.gpkg (GeoPackage)")


if __name__ == "__main__":
    main()
