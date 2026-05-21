"""
OpenDQI Spark interop — EMIR Data Quality Pack wrapper
(v0.15.0, **EXPERIMENTAL**).

Provides ``opendqi.spark.emir.data_quality_pack(*, tsr=None,
tar=None, msr=None, mar=None, feedback=None, mappings=None,
as_of=None)`` — the Spark-DataFrame-aware counterpart of
``opendqi.emir.data_quality_pack``.

**Mechanism: collect-then-call.** Each provided Spark
DataFrame is materialized at the driver via ``.toPandas()`` →
``pyarrow.Table.from_pandas(...)`` → handed to the core Rust
``opendqi.emir.data_quality_pack`` Arrow-input path. The
result is a fully-resident ``PyDqiPackResult`` ready for
``.indicators`` / ``.evidence`` / ``.issues`` /
``.summary``.

**Honest limitations** :
- All inputs are **collected to the driver** ; this does not
  scale beyond driver-RAM. Native partition-aware joins
  (TSR ↔ MSR via Spark) are scheduled for v0.16.
- ``mar`` is paths-only in v0.15 — passing a Spark
  DataFrame for ``mar`` raises ValueError.
- A ``FutureWarning`` is emitted on every call to flag the
  experimental status.

PySpark is **not** declared as a runtime dependency — duck-
typed import inside the function. Install via:

    pip install opendqi[spark]
"""
from __future__ import annotations

import warnings
from typing import TYPE_CHECKING, Any, Mapping, Optional

if TYPE_CHECKING:  # pragma: no cover
    from pyspark.sql import DataFrame as SparkDataFrame

import opendqi


def data_quality_pack(
    *,
    tsr: "Optional[Any]" = None,
    tar: "Optional[Any]" = None,
    msr: "Optional[Any]" = None,
    mar: "Optional[str]" = None,
    feedback: "Optional[Any]" = None,
    mappings: "Optional[Mapping[str, Mapping[str, str]]]" = None,
    as_of: "Optional[str]" = None,
):
    """Run the EMIR Data Quality Pack from Spark DataFrame inputs.

    Each layer accepts ``None``, a ``str`` (XML file path), or a
    ``pyspark.sql.DataFrame``. Spark DataFrames are collected at
    the driver, converted to ``pyarrow.Table`` via pandas, then
    forwarded to ``opendqi.emir.data_quality_pack``. The
    returned ``PyDqiPackResult`` is identical to what the core
    bindings produce — same Arrow contracts, same
    ``.summary``, same ``.report(out_dir)`` method.

    Parameters
    ----------
    tsr, tar, msr, feedback :
        Either ``None``, an ``str`` XML path, OR a
        ``pyspark.sql.DataFrame``. When a Spark DataFrame is
        provided, ``mappings[layer]`` MUST contain the
        ``canonical_field → spark_column_name`` map.
    mar :
        ``str`` XML path only in v0.15 (Spark DataFrame input
        for MAR will be added in v0.16). Passing a Spark
        DataFrame here raises ``ValueError`` at the core layer.
    mappings :
        ``{"tsr": {"uti": "TradeId", ...}, "tar": {...}, ...}``
        — only the layers passed as Spark DataFrames need an
        entry. File-path inputs ignore ``mappings``.
    as_of :
        ``YYYY-MM-DD`` reference date for age-based indicators.
        Defaults to today (UTC).

    Returns
    -------
    PyDqiPackResult
        Same type as ``opendqi.emir.data_quality_pack`` ;
        ``.indicators`` / ``.evidence`` / ``.issues`` /
        ``.summary`` / ``.report(out_dir)``.

    Warnings
    --------
    Always emits a ``FutureWarning`` flagging the experimental
    status and the driver-side collect.
    """
    warnings.warn(
        "opendqi.spark.emir.data_quality_pack is EXPERIMENTAL "
        "(v0.15.0 collect-then-call ; native partition-aware = v0.16).",
        FutureWarning,
        stacklevel=2,
    )

    def _collect(obj):
        """Spark DataFrame → pyarrow.Table ; str → str
        passthrough ; None → None."""
        if obj is None:
            return None
        if isinstance(obj, str):
            return obj
        # Duck-typed: anything with `.toPandas()` is a Spark
        # DataFrame.
        if hasattr(obj, "toPandas"):
            import pyarrow as pa

            pdf = obj.toPandas()
            return pa.Table.from_pandas(pdf, preserve_index=False)
        # pyarrow.Table or RecordBatch — pass through unchanged.
        return obj

    return opendqi.emir.data_quality_pack(
        tsr=_collect(tsr),
        tar=_collect(tar),
        msr=_collect(msr),
        mar=mar,  # paths-only at this stage
        feedback=_collect(feedback),
        mappings=dict(mappings) if mappings is not None else None,
        as_of=as_of,
    )


__all__ = ["data_quality_pack"]
