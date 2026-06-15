import duckdb


def main() -> None:
    conn = duckdb.connect()

    conn.execute("INSTALL spatial")
    conn.execute("INSTALL httpfs")
    conn.execute("LOAD spatial")
    conn.execute("LOAD httpfs")

    conn.execute("SET s3_region='us-west-2'")

    conn.execute(
        """
        COPY (
            SELECT
                *
            FROM read_parquet('s3://overturemaps-us-west-2/release/2026-05-20.0/theme=transportation/type=segment/*')
            WHERE
                bbox.xmin > 51.5835 AND bbox.xmax < 51.8528 AND
                bbox.ymin > -1.4973 AND bbox.ymax < -0.9893
        )
        TO 'data/oxford_segments.parquet'
        """
    )

    print("Data exported successfully to data/oxford_segments.parquet")


if __name__ == "__main__":
    main()
