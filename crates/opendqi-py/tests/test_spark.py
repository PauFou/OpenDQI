"""
v0.14.0 — `opendqi.spark.scan_spark_dataframe` native
mapInPandas (promoted from v0.13 experimental).

Tests requiring PySpark use `pytest.importorskip`, so the
standard `pytest tests/` run on a fresh venv (no PySpark)
still passes the smoke tests.
"""
from __future__ import annotations

import warnings

import pytest


# --- Tests that DON'T need PySpark --------------------------------


def test_spark_module_importable() -> None:
    """`import opendqi.spark` works even without pyspark installed."""
    from opendqi import spark
    assert callable(spark.scan_spark_dataframe)
    assert spark.__doc__ is not None
    # v0.14: status is "native" (no longer experimental). The
    # docstring may still reference the v0.13 experimental history
    # in passing — we check for the positive marker instead of
    # absence of the word "experimental".
    assert "native" in spark.__doc__.lower() or "mapInPandas" in spark.__doc__, (
        f"v0.14 docstring should mention native or mapInPandas; "
        f"got: {spark.__doc__[:200]}..."
    )


def test_spark_top_level_attribute() -> None:
    """opendqi.spark is reachable as a top-level attribute too."""
    import opendqi
    assert hasattr(opendqi, "spark")
    assert callable(opendqi.spark.scan_spark_dataframe)


def test_spark_unknown_regime_raises_value_error() -> None:
    """`regime='whatever'` raises ValueError before any Spark call."""
    from opendqi.spark import scan_spark_dataframe

    class FakeDataFrame:
        pass

    with pytest.raises(ValueError) as exc_info:
        scan_spark_dataframe(
            FakeDataFrame(),  # type: ignore[arg-type]
            regime="unknown_regime",
            mapping={"uti": "u"},
        )
    msg = str(exc_info.value)
    assert "unknown_regime" in msg


def test_spark_no_longer_emits_future_warning() -> None:
    """v0.14: opendqi.spark is no longer experimental; calling
    scan_spark_dataframe with a bad regime should NOT emit any
    FutureWarning (the v0.13 deprecation warning is gone)."""
    from opendqi.spark import scan_spark_dataframe

    class FakeDataFrame:
        pass

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        with pytest.raises(ValueError):
            scan_spark_dataframe(
                FakeDataFrame(),  # type: ignore[arg-type]
                regime="bad",
                mapping={},
            )
    # ZERO FutureWarnings should have been emitted.
    future_warnings = [w for w in caught if issubclass(w.category, FutureWarning)]
    assert not future_warnings, (
        f"v0.14 opendqi.spark should not emit FutureWarning; "
        f"got {[str(w.message) for w in future_warnings]}"
    )


# --- Tests that REQUIRE PySpark installed -------------------------


pyspark = pytest.importorskip("pyspark")


@pytest.fixture(scope="module")
def spark_session():
    """Session-scoped SparkSession to amortize the ~10-20s startup
    cost. Stopped at module teardown to free threads.

    Skips the test cleanly if a SparkSession can't be created
    (most commonly: Java/JDK not installed on the dev machine).
    """
    try:
        from pyspark.sql import SparkSession
        s = (
            SparkSession.builder
            .master("local[2]")
            .appName("opendqi-py-test")
            .config("spark.sql.shuffle.partitions", "2")
            .config("spark.sql.execution.arrow.pyspark.enabled", "true")
            .getOrCreate()
        )
        s.sparkContext.setLogLevel("WARN")
    except Exception as e:
        pytest.skip(
            f"could not start SparkSession (Java/JDK missing?): "
            f"{type(e).__name__}: {e}"
        )
    yield s
    s.stop()


def test_spark_native_returns_spark_dataframe(spark_session) -> None:
    """scan_spark_dataframe returns a Spark DataFrame with the v1.0
    issues schema (11 string columns)."""
    import opendqi
    from pyspark.sql.types import StringType

    # Build a tiny Spark DataFrame that looks like an EMIR submission.
    sdf = spark_session.createDataFrame(
        [("U1", None), ("U2", None), ("U3", None)],
        ["trade_uti", "_dummy"],
    )
    result_sdf = opendqi.spark.scan_spark_dataframe(
        sdf,
        regime="emir",
        mapping={"uti": "trade_uti"},
    )

    # Returns a Spark DataFrame (not pandas, not None).
    assert hasattr(result_sdf, "schema")
    assert hasattr(result_sdf, "count")

    # 11 columns, names match the v1.0 contract, all String types.
    expected_cols = [
        "check_id", "regime", "severity", "dimension",
        "record_id", "uti", "field", "value",
        "message", "source_file", "evidence_json",
    ]
    assert result_sdf.columns == expected_cols
    for col in expected_cols:
        assert isinstance(
            result_sdf.schema[col].dataType, StringType
        ), f"col {col!r} not StringType: {result_sdf.schema[col].dataType}"


def test_spark_native_produces_issues_on_minimal_input(spark_session) -> None:
    """A 3-row input with only UTI mapped produces real EMIR.COMP.* issues
    (every unmapped canonical field surfaces as missing)."""
    import opendqi

    sdf = spark_session.createDataFrame(
        [("U1",), ("U2",), ("U3",)],
        ["trade_uti"],
    )
    issues_sdf = opendqi.spark.scan_spark_dataframe(
        sdf, regime="emir", mapping={"uti": "trade_uti"}
    )
    issues_sdf.cache()
    # Many EMIR.COMP.* checks fire (all unmapped fields show up as
    # missing). Each of 3 rows triggers ~20 checks.
    total = issues_sdf.count()
    assert total > 0
    # Spot-check: at least one EMIR.* issue is in there.
    check_ids = [r["check_id"] for r in issues_sdf.limit(20).collect()]
    assert any(c.startswith("EMIR.") for c in check_ids), (
        f"no EMIR.* check_id in first 20 issues; got: {check_ids}"
    )
