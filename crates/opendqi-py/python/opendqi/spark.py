"""
OpenDQI Spark interop (v0.14.0 — native).

Partition-friendly Spark integration via ``DataFrame.mapInPandas``:
each Spark partition becomes a pandas chunk, runs through
``opendqi.{emir,sftr}.scan_table`` independently (so the scan
stays distributed — no full collect to the driver), and the
issues stream back as a Spark DataFrame of the **v1.0 stable
11-column issues schema**.

This is the v0.14.0 replacement for the v0.13.0 experimental
helper that did a single ``df.toPandas()`` collect at the
driver. The signature is unchanged; the behaviour is
distributed-safe, and the ``FutureWarning`` is dropped.

PySpark is **not** declared as a runtime dependency of the
``opendqi`` wheel — duck-typed import inside the helper. Users
opt in via:

    pip install opendqi[spark]
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Mapping

if TYPE_CHECKING:  # pragma: no cover
    from pyspark.sql import DataFrame as SparkDataFrame

import opendqi


# The v1.0 issues schema, mirrored as Spark types via the
# Spark DDL string format. All columns are nullable=true except
# the required ones (matching `issues_schema()` in
# crates/opendqi-py/src/issues.rs).
_SPARK_ISSUES_SCHEMA_DDL = (
    "check_id STRING NOT NULL, "
    "regime STRING NOT NULL, "
    "severity STRING NOT NULL, "
    "dimension STRING NOT NULL, "
    "record_id STRING, "
    "uti STRING, "
    "field STRING, "
    "value STRING, "
    "message STRING NOT NULL, "
    "source_file STRING, "
    "evidence_json STRING"
)


def scan_spark_dataframe(
    df: "SparkDataFrame",
    *,
    regime: str = "emir",
    mapping: Mapping[str, str],
    normalize: bool = False,
) -> "SparkDataFrame":
    """Run an opendqi scan on a Spark DataFrame, partition-wise.

    Uses ``DataFrame.mapInPandas`` so each Spark partition is
    scanned independently — no full collect to the driver, the
    work stays distributed across executors. Returns a Spark
    DataFrame of the v1.0 stable 11-column issues schema (all
    StringType, nullability matches
    ``opendqi.PyScanResult.issues``).

    Parameters
    ----------
    df :
        A ``pyspark.sql.DataFrame`` whose columns include the
        values of ``mapping``.
    regime :
        ``"emir"`` or ``"sftr"``.
    mapping :
        ``canonical_field`` → ``spark_column_name``. Same
        direction as :func:`opendqi.emir.scan_table` and the
        CLI's ``--mapping`` YAML.
    normalize :
        Passthrough to ``scan_table`` (ignored here — this
        function returns issues only, not normalized records).

    Returns
    -------
    pyspark.sql.DataFrame
        The issues table, ready to be ``.write.parquet(...)``-ed
        or joined back to the input.

    Notes
    -----
    Schema-of-record is the v1.0 stable contract from
    :func:`opendqi.PyScanResult.issues` — 11 String columns,
    same order as the CLI's ``issues.csv``. Mismatches between
    the input column dtypes and what ``scan_table`` expects
    (Decimal128(38,10), Date32, Timestamp(μs,UTC), ...) raise
    inside the executor — cast in PySpark before calling this
    helper if needed.
    """
    if regime not in ("emir", "sftr"):
        raise ValueError(
            f"unknown regime: {regime!r} (expected 'emir' or 'sftr')"
        )

    # Capture the args by value so the closure is serializable
    # across the Spark wire (pickle).
    mapping_dict = dict(mapping)
    regime_name = regime

    import pyarrow as pa

    def _scan_partition(iter_pdf):
        """mapInPandas executor body: yields one issues DataFrame
        per input partition chunk."""
        for pdf in iter_pdf:
            # pandas → Arrow (cheap on most dtype combos —
            # zero-copy for numeric / string / datetime types).
            table = pa.Table.from_pandas(pdf, preserve_index=False)
            if regime_name == "emir":
                result = opendqi.emir.scan_table(
                    table, mapping_dict, normalize=normalize
                )
            else:  # "sftr" — validated above
                result = opendqi.sftr.scan_table(
                    table, mapping_dict, normalize=normalize
                )
            # Arrow → pandas for the mapInPandas return type.
            # `to_pandas()` is cheap (mostly string columns).
            yield result.issues.to_pandas()

    return df.mapInPandas(_scan_partition, schema=_SPARK_ISSUES_SCHEMA_DDL)


__all__ = ["scan_spark_dataframe"]
