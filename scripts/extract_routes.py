import duckdb

# Connect to DuckDB
conn = duckdb.connect()

# Load extensions
conn.execute("INSTALL spatial")
conn.execute("INSTALL httpfs")
conn.execute("LOAD spatial")
conn.execute("LOAD httpfs")

# Set S3 configuration
conn.execute("SET s3_region='us-west-2'")

# Execute query and export to parquet
conn.execute("""
COPY (
    SELECT
        *
    FROM read_parquet('s3://overturemaps-us-west-2/release/2026-05-20.0/theme=transportation/type=segment/*')
    WHERE
        bbox.xmin > 51.5835 AND bbox.xmax < 51.8528 AND
        bbox.ymin > -1.4973 AND bbox.ymax < -0.9893
)
TO 'data/rome_segments.parquet'
""")

print("Data exported successfully to data/rome_segments.parquet")