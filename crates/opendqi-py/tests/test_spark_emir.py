"""
v0.15.0 — `opendqi.spark.emir.data_quality_pack` EXPERIMENTAL
wrapper.

Collect-then-call adapter: each provided Spark DataFrame is
materialized at the driver via `.toPandas()` →
`pyarrow.Table.from_pandas(...)` → handed to the core Rust
`opendqi.emir.data_quality_pack`.

These tests check:
- the FutureWarning is emitted on every call (experimental status)
- the duck-typed `.toPandas()` detection works (no PySpark required
  for the basic adapter logic — we mock with a stand-in object)
- string paths pass through unchanged (mix-and-match)

PySpark itself is optional ; the JVM-bound end-to-end test is
gated behind `pytest.importorskip("pyspark")` AND a try/except
SparkSession bootstrap (skips cleanly when Java is missing on
the dev machine — mirrors test_spark.py's pattern).
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
TSR_FIXTURE = REPO_ROOT / "examples" / "quickstart-emir" / "auth107-tsr.xml"
FBK_FIXTURE = REPO_ROOT / "examples" / "quickstart-emir" / "auth092-feedback.xml"


def test_submodule_importable() -> None:
    """opendqi.spark migrated from flat to package — both the
    new submodule and the old top-level function stay imported."""
    import opendqi.spark
    import opendqi.spark.emir

    assert hasattr(opendqi.spark, "scan_spark_dataframe"), "flat name dropped"
    assert hasattr(opendqi.spark, "emir"), "emir submodule missing"
    assert hasattr(opendqi.spark.emir, "data_quality_pack"), (
        "data_quality_pack missing"
    )


def test_backward_compat_flat_imports_still_work() -> None:
    """`from opendqi.spark import scan_spark_dataframe` must
    continue to import (the v0.14 surface)."""
    from opendqi.spark import scan_spark_dataframe  # noqa: F401

    assert callable(scan_spark_dataframe)


def test_string_paths_passthrough_emits_future_warning(tmp_path: Path) -> None:
    """When all inputs are string paths, the wrapper behaves as
    a thin call into opendqi.emir.data_quality_pack but ALWAYS
    emits FutureWarning (experimental status)."""
    if not TSR_FIXTURE.exists():
        pytest.skip("quickstart TSR fixture missing")
    import opendqi.spark.emir

    with pytest.warns(FutureWarning, match="EXPERIMENTAL"):
        result = opendqi.spark.emir.data_quality_pack(
            tsr=str(TSR_FIXTURE),
            as_of="2026-05-21",
        )
    # Same result type as the core function — same getters work.
    assert result.indicators.num_rows == 17
    assert result.summary["files_processed"] == 1


def test_at_least_one_input_required_propagates() -> None:
    """The core's precondition propagates through the wrapper."""
    import opendqi.spark.emir

    with pytest.warns(FutureWarning):
        with pytest.raises(ValueError, match="at least one"):
            opendqi.spark.emir.data_quality_pack(as_of="2026-05-21")


def test_duck_typed_collect_with_stand_in_object() -> None:
    """The wrapper uses duck-typing for Spark DataFrame
    detection (``hasattr(obj, 'toPandas')``). Verify that a
    minimal stand-in carrying a `.toPandas()` method is
    correctly collected — proves the dispatch logic doesn't
    rely on a real PySpark import."""
    import pandas as pd
    import opendqi.spark.emir

    class FakeSparkDf:
        """Minimal duck-type matching what the wrapper needs."""

        def __init__(self, pdf: pd.DataFrame):
            self._pdf = pdf

        def toPandas(self) -> pd.DataFrame:
            return self._pdf

    # Build a TSR-shaped fake Spark DataFrame.
    import pyarrow as pa  # noqa: F401  — used implicitly by core

    fake_tsr = FakeSparkDf(
        pd.DataFrame(
            {
                "TradeId": ["U1", "U2"],
                "Status": ["OUTSTANDING", "OUTSTANDING"],
            }
        )
    )

    with pytest.warns(FutureWarning):
        result = opendqi.spark.emir.data_quality_pack(
            tsr=fake_tsr,
            mappings={"tsr": {"uti": "TradeId", "status": "Status"}},
            as_of="2026-05-21",
        )
    # Both rows have no valuation_amount mapped → DQI_VAL_MISSING
    # is computed and reports 2/2.
    df = result.indicators.to_pandas()
    val = df[df["indicator_id"] == "DQI_VAL_MISSING"].iloc[0]
    assert val["denominator"] == 2
    assert val["numerator"] == 2
    assert val["status"] == "red"


def test_mar_spark_dataframe_rejected_with_clear_error() -> None:
    """MAR is paths-only in v0.15 — the core's ValueError
    propagates with a clear v0.15 message."""
    import pandas as pd
    import opendqi.spark.emir

    class FakeSparkDf:
        def toPandas(self):
            return pd.DataFrame({"x": [1]})

    # `mar` is documented as str-only ; we pass a Spark-shaped
    # object directly to expose the underlying ValueError. The
    # wrapper passes `mar=mar` unchanged (no `_collect` for
    # MAR), so this surfaces the core's check.
    with pytest.warns(FutureWarning):
        with pytest.raises((ValueError, TypeError)):
            opendqi.spark.emir.data_quality_pack(
                mar=FakeSparkDf(),
                as_of="2026-05-21",
            )


# -------------------------------------------------------------------
# Optional Spark integration test — skipped without PySpark + JVM
# -------------------------------------------------------------------


@pytest.fixture(scope="module")
def spark_session():
    pyspark = pytest.importorskip("pyspark")
    try:
        from pyspark.sql import SparkSession

        s = (
            SparkSession.builder.master("local[1]")
            .appName("opendqi-test-spark-emir")
            .config("spark.sql.shuffle.partitions", "1")
            .getOrCreate()
        )
        s.sparkContext.setLogLevel("WARN")
        yield s
        s.stop()
    except Exception as e:  # pragma: no cover — needs a JVM
        pytest.skip(f"could not start SparkSession (Java missing?): {e}")


def test_real_spark_dataframe_end_to_end(spark_session) -> None:
    """End-to-end with a real local SparkSession + a tiny
    Spark DataFrame. Skipped automatically on dev machines
    without Java/JDK."""
    import opendqi.spark.emir

    sdf = spark_session.createDataFrame(
        [("U1", "OUTSTANDING"), ("U2", "OUTSTANDING")],
        ["TradeId", "Status"],
    )

    with pytest.warns(FutureWarning):
        result = opendqi.spark.emir.data_quality_pack(
            tsr=sdf,
            mappings={"tsr": {"uti": "TradeId", "status": "Status"}},
            as_of="2026-05-21",
        )
    df = result.indicators.to_pandas()
    val = df[df["indicator_id"] == "DQI_VAL_MISSING"].iloc[0]
    assert val["denominator"] == 2
    assert val["status"] == "red"
