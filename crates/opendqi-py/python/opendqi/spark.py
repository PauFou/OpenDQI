"""
OpenDQI Spark interop — experimental (v0.13.0).

Pure-Python helper that round-trips a PySpark DataFrame through
Arrow into ``opendqi.{emir,sftr}.scan_table``, returning a Spark
DataFrame of the issues table (v1.0 stable 11-column schema).

**No PySpark dependency is declared** in the wheel: this module
uses duck typing. Users who want to call ``scan_spark_dataframe``
install ``pyspark`` themselves; users who don't never touch this
module.

Status
------
**EXPERIMENTAL.** This helper does a Spark → pandas → Arrow
round-trip; on PySpark 3.x that's the only portable path. The
signature may evolve in v0.14 once a native ``opendqi.spark``
namespace with ``mapInPandas`` UDFs ships (zero-copy via Arrow
column batches, partition-friendly). Use at your own risk in
production pipelines.

A ``FutureWarning`` is emitted on every call to make the
preview status visible in operator logs.
"""
from __future__ import annotations

import warnings
from typing import TYPE_CHECKING, Mapping

if TYPE_CHECKING:  # pragma: no cover
    from pyspark.sql import DataFrame as SparkDataFrame

import opendqi


def scan_spark_dataframe(
    df: "SparkDataFrame",
    *,
    regime: str = "emir",
    mapping: Mapping[str, str],
    normalize: bool = False,
) -> "SparkDataFrame":
    """Run an opendqi scan on a Spark DataFrame.

    Parameters
    ----------
    df :
        A ``pyspark.sql.DataFrame`` whose columns match the
        values of ``mapping``.
    regime :
        ``"emir"`` or ``"sftr"``. Selects which check pack runs
        (89 EMIR or 44 SFTR single-batch checks).
    mapping :
        ``canonical_field`` → ``spark_column_name``. Same direction
        as :func:`opendqi.emir.scan_table` and the CLI's
        ``--mapping`` YAML.
    normalize :
        Passthrough to ``scan_table`` — when True, the result also
        carries the canonical-model Arrow batch via
        ``result.normalized`` (here unused since this function
        returns issues only).

    Returns
    -------
    pyspark.sql.DataFrame
        The issues table with the v1.0 stable 11-column schema
        (``check_id``, ``regime``, ``severity``, ``dimension``,
        ``record_id``, ``uti``, ``field``, ``value``, ``message``,
        ``source_file``, ``evidence_json``). All columns are
        Spark ``StringType``.

    Notes
    -----
    The round-trip is Spark → pandas → Arrow → opendqi.scan_table
    → pandas → Spark. On PySpark 3.x that's the only portable
    path (PySpark 4.0+ has ``df.toArrow()`` for the zero-copy
    forward direction, but the reverse — ``spark.createDataFrame
    `` from Arrow — still goes through pandas). The pure round-
    trip overhead dominates only for very large DataFrames; for
    those, prefer the CLI handoff pattern:

    .. code-block:: python

        spark_df.write.parquet("/tmp/in")
        # `opendqi emir scan /tmp/in --out /tmp/out` (CLI)
        spark.read.csv("/tmp/out/issues.csv", header=True)
    """
    warnings.warn(
        "opendqi.spark is experimental in v0.13. Signature may "
        "evolve in v0.14 once a native mapInPandas UDF ships. "
        "Use at your own risk in production pipelines.",
        FutureWarning,
        stacklevel=2,
    )

    # Fail-fast on bad inputs BEFORE the expensive Spark →
    # pandas round-trip (so users with a typo'd regime get a
    # clean error in milliseconds instead of after the data
    # collect).
    if regime not in ("emir", "sftr"):
        raise ValueError(
            f"unknown regime: {regime!r} (expected 'emir' or 'sftr')"
        )

    import pyarrow as pa  # imported here so the warning fires
                          # even if pyarrow is missing (rare)

    # Spark DataFrame → pandas → Arrow Table.
    # On PySpark 4.0+ this could be `df.toArrow()` (zero-copy),
    # but we target the 3.x baseline most users are still on.
    pdf = df.toPandas()
    table = pa.Table.from_pandas(pdf, preserve_index=False)

    if regime == "emir":
        result = opendqi.emir.scan_table(
            table, dict(mapping), normalize=normalize
        )
    else:  # regime == "sftr" (validated above)
        result = opendqi.sftr.scan_table(
            table, dict(mapping), normalize=normalize
        )

    # Issues table → pandas → Spark DataFrame. Uses the
    # existing SparkSession bound to the input DataFrame.
    spark = df.sparkSession  # type: ignore[attr-defined]
    issues_pdf = result.issues.to_pandas()
    return spark.createDataFrame(issues_pdf)


__all__ = ["scan_spark_dataframe"]
