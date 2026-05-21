"""
v0.13.0 — `opendqi.spark` experimental Spark interop.

Pure-Python helper, no PySpark dependency declared. Tests below
exercise the module-loading + FutureWarning emission without
actually instantiating a SparkSession (PySpark isn't installed
in the dev venv).
"""
from __future__ import annotations

import warnings

import pytest


def test_spark_module_importable() -> None:
    """`import opendqi.spark` works even without pyspark installed."""
    from opendqi import spark
    assert callable(spark.scan_spark_dataframe)
    assert spark.__doc__ is not None
    assert "EXPERIMENTAL" in spark.__doc__


def test_spark_top_level_attribute() -> None:
    """opendqi.spark is reachable as a top-level attribute too."""
    import opendqi
    assert hasattr(opendqi, "spark")
    assert callable(opendqi.spark.scan_spark_dataframe)


def test_spark_unknown_regime_raises_value_error() -> None:
    """`regime='whatever'` should raise ValueError (not warn-then-crash)."""
    from opendqi.spark import scan_spark_dataframe

    # A mock that quacks like a Spark DataFrame just enough for the
    # function to fail on the regime check (which is BEFORE the
    # pyspark/pyarrow path).
    class FakeDataFrame:
        def toPandas(self):
            raise AssertionError("should not be called — regime check fires first")

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        with pytest.raises(ValueError) as exc_info:
            scan_spark_dataframe(
                FakeDataFrame(),  # type: ignore[arg-type]
                regime="unknown_regime",
                mapping={"uti": "u"},
            )
        # The FutureWarning is emitted FIRST (before the regime check).
        assert any(
            issubclass(w.category, FutureWarning)
            for w in caught
        ), f"expected a FutureWarning; got {[w.category for w in caught]}"

    msg = str(exc_info.value)
    assert "unknown_regime" in msg
    assert "emir" in msg.lower() or "sftr" in msg.lower()


def test_spark_emits_future_warning_with_v013_message() -> None:
    """The warning text mentions v0.13 + experimental."""
    from opendqi.spark import scan_spark_dataframe

    class FakeDataFrame:
        def toPandas(self):
            raise AssertionError

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        try:
            scan_spark_dataframe(
                FakeDataFrame(),  # type: ignore[arg-type]
                regime="not_a_regime",
                mapping={},
            )
        except ValueError:
            pass

    relevant = [w for w in caught if issubclass(w.category, FutureWarning)]
    assert relevant, f"no FutureWarning emitted; caught={[w.category for w in caught]}"
    msg = str(relevant[0].message)
    assert "experimental" in msg.lower()
    assert "v0.13" in msg or "v0.14" in msg
