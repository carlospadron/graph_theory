import duckdb

def test_cluster_stats():
    conn = duckdb.connect()
    
    stats = conn.execute("""
        SELECT cluster_id, COUNT(*) as building_count 
        FROM read_csv_auto('data/rust_clusters.csv') 
        GROUP BY cluster_id 
        ORDER BY building_count DESC
    """).fetchdf()
    
    # Basic assertions to verify the output makes sense
    assert len(stats) > 0, "No clusters were generated"
    assert "cluster_id" in stats.columns
    assert "building_count" in stats.columns
    
    # We know the largest cluster should have several thousand buildings now
    assert stats["building_count"].max() > 1000, "Expected a large contiguous cluster"
    
    # Verify we aren't generating more clusters than buildings
    # (since many group together, we expect the count to be much less than ~160k)
    assert len(stats) < 100000, "Too many clusters generated (linkage might be failing)"
