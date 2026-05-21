"""
OpenDQI Spark interop (v0.14.0 — native scan_spark_dataframe ;
v0.15.0 — added opendqi.spark.emir submodule with the
data_quality_pack collect-then-call wrapper).

Partition-friendly Spark integration via ``DataFrame.mapInPandas``:
each Spark partition becomes a pandas chunk, runs through
``opendqi.{emir,sftr}.scan_table`` independently (so the scan
stays distributed — no full collect to the driver), and the
issues stream back as a Spark DataFrame of the **v1.0 stable
11-column issues schema**.

PySpark is **not** declared as a runtime dependency of the
``opendqi`` wheel — duck-typed import inside the helper. Users
opt in via:

    pip install opendqi[spark]

v0.15.0 migration : ``opendqi.spark`` was promoted from a flat
module to a package so that ``opendqi.spark.emir`` (and future
``.sftr``) can live alongside the existing
``scan_spark_dataframe``. The public surface is unchanged —
``from opendqi.spark import scan_spark_dataframe`` continues
to work.
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
    DataFrame of the v1.0 stable 11-column issues schema.

    Parameters
    ----------
    df :
        A ``pyspark.sql.DataFrame`` whose columns include the
        values of ``mapping``.
    regime :
        ``"emir"`` or ``"sftr"``.
    mapping :
        ``canonical_field`` → ``spark_column_name``.
    normalize :
        Passthrough to ``scan_table`` (ignored here — this
        function returns issues only).
    """
    if regime not in ("emir", "sftr"):
        raise ValueError(
            f"unknown regime: {regime!r} (expected 'emir' or 'sftr')"
        )

    mapping_dict = dict(mapping)
    regime_name = regime

    import pyarrow as pa

    def _scan_partition(iter_pdf):
        for pdf in iter_pdf:
            table = pa.Table.from_pandas(pdf, preserve_index=False)
            if regime_name == "emir":
                result = opendqi.emir.scan_table(
                    table, mapping_dict, normalize=normalize
                )
            else:
                result = opendqi.sftr.scan_table(
                    table, mapping_dict, normalize=normalize
                )
            yield result.issues.to_pandas()

    return df.mapInPandas(_scan_partition, schema=_SPARK_ISSUES_SCHEMA_DDL)


# v0.15.0 — opendqi.spark.emir submodule (data_quality_pack).
# Lazy import to keep `import opendqi.spark` cheap when only
# scan_spark_dataframe is needed.
from . import emir  # noqa: F401,E402


__all__ = ["scan_spark_dataframe", "emir"]
