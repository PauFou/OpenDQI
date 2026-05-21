#!/usr/bin/env python3
"""
OpenDQI Python quickstart — Pattern 6 (v0.14.0): native Spark
mapInPandas UDF.

Drops the v0.13 round-trip via `df.toPandas()` (full collect at
driver) in favour of partition-wise scanning via Spark's native
`mapInPandas`. The work stays distributed across executors; the
issues stream back as a Spark DataFrame of the v1.0 stable
11-column issues schema.

Requires: pip install opendqi[spark]   (PySpark + a JVM)
Run from the repo root:

    python examples/python/06_spark_mapInPandas.py
"""
from __future__ import annotations

from pathlib import Path

import opendqi


def main() -> None:
    try:
        from pyspark.sql import SparkSession
    except ImportError:
        raise SystemExit(
            "✗ pyspark not installed. Run: pip install opendqi[spark]"
        )

    # Local 2-thread Spark session (enough for the demo; in
    # production this would be your cluster session).
    try:
        spark = (
            SparkSession.builder
            .master("local[2]")
            .appName("opendqi-mapInPandas-demo")
            .config("spark.sql.shuffle.partitions", "2")
            .config("spark.sql.execution.arrow.pyspark.enabled", "true")
            .getOrCreate()
        )
        spark.sparkContext.setLogLevel("WARN")
    except Exception as e:
        raise SystemExit(
            f"✗ could not start SparkSession (Java/JDK missing?):\n  {e}"
        )

    # Build a tiny EMIR-shaped Spark DataFrame. In a real
    # pipeline this would come from `spark.read.parquet(...)`,
    # a Delta table, a Hive query, etc.
    sdf = spark.createDataFrame(
        [
            ("U001", "DUMMYLEI001"),
            ("U002", "DUMMYLEI002"),
            ("U003", "DUMMYLEI003"),
            ("U004", "DUMMYLEI004"),
            ("U005", "DUMMYLEI005"),
        ],
        ["trade_uti", "lei"],
    )
    print(f"input Spark DataFrame: {sdf.count()} rows, {len(sdf.columns)} cols")

    # The scan: mapInPandas runs partition-by-partition (not full
    # collect to driver). Returns a Spark DataFrame of the v1.0
    # stable 11-column issues schema.
    issues_sdf = opendqi.spark.scan_spark_dataframe(
        sdf,
        regime="emir",
        mapping={
            "uti":                              "trade_uti",
            "entity_responsible_for_reporting": "lei",
        },
    )

    # The result is a Spark DataFrame — chain Spark ops on it.
    print(f"\nissues schema: {issues_sdf.schema.simpleString()}")
    print(f"\ntotal issues: {issues_sdf.count()}")

    print("\ntop 5 check_ids:")
    (
        issues_sdf.groupBy("check_id")
        .count()
        .orderBy("count", ascending=False)
        .show(5, truncate=False)
    )

    spark.stop()


if __name__ == "__main__":
    main()
