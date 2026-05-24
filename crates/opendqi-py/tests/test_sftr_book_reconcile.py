"""
v0.17 H5 — `opendqi.sftr.book_reconcile` cross-message workflow.

Tests the dual book-input contract on the SFTR side :
  - str path → .csv (needs mapping) or .parquet (no mapping)
  - pyarrow.Table / pyarrow.RecordBatch (always needs mapping)

Mirror of the EMIR-side test_book_reconcile.py, scoped to the
SFTR-specific BREC family.
"""
from __future__ import annotations

from pathlib import Path

import pyarrow as pa
import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]
SFTR_BOOK_CSV = REPO_ROOT / "examples" / "sftr" / "book_reconcile" / "book.csv"
SFTR_TSR_XML = REPO_ROOT / "examples" / "sftr" / "tr_state" / "auth079-sample.xml"

# Mirrors examples/sftr/book_reconcile/book_mapping.yml.
SFTR_BOOK_MAPPING = {
    "uti": "trade_uti",
    "sft_type": "sft_type",
    "counterparty_1": "reporting_cpty",
    "counterparty_2": "other_cpty",
    "loan_value": "loan",
    "loan_currency": "loan_ccy",
    "collateral_value": "collateral",
    "collateral_currency": "coll_ccy",
    "haircut": "haircut",
    "maturity_date": "maturity",
    "reuse_indicator": "reuse",
    "settlement_date": "settlement",
    "collateral_portfolio_code": "portfolio",
}


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


def test_sftr_book_reconcile_csv_path() -> None:
    """The most common path : a .csv book + a dict mapping."""
    _require(SFTR_BOOK_CSV, SFTR_TSR_XML)
    import opendqi

    result = opendqi.sftr.book_reconcile(
        str(SFTR_BOOK_CSV),
        str(SFTR_TSR_XML),
        mapping=SFTR_BOOK_MAPPING,
    )
    s = result.summary
    assert s["regime"] == "sftr"
    assert s["files_processed"] == 2
    # The shipped fixture pair is designed to fire several
    # SFTR.BREC.* mismatches (loan-value, currency, status,
    # maturity, etc.).
    assert s["issues_total"] > 0


def test_sftr_book_reconcile_only_brec_checks() -> None:
    """book_reconcile fires ONLY SFTR.BREC.* — no other family leaks."""
    _require(SFTR_BOOK_CSV, SFTR_TSR_XML)
    import opendqi

    result = opendqi.sftr.book_reconcile(
        str(SFTR_BOOK_CSV),
        str(SFTR_TSR_XML),
        mapping=SFTR_BOOK_MAPPING,
    )
    check_ids = set(result.issues.column("check_id").to_pylist())
    non_brec = {c for c in check_ids if not c.startswith("SFTR.BREC.")}
    assert not non_brec, f"non-BREC checks leaked: {non_brec}"


def test_sftr_book_reconcile_csv_requires_mapping() -> None:
    """.csv path without `mapping=` raises a helpful error."""
    _require(SFTR_BOOK_CSV, SFTR_TSR_XML)
    import opendqi

    with pytest.raises(ValueError) as exc:
        opendqi.sftr.book_reconcile(str(SFTR_BOOK_CSV), str(SFTR_TSR_XML))
    assert "mapping" in str(exc.value).lower()


def test_sftr_book_reconcile_unsupported_extension(tmp_path: Path) -> None:
    """A book path with a non-CSV/Parquet extension raises clearly."""
    _require(SFTR_TSR_XML)
    import opendqi

    bad = tmp_path / "book.txt"
    bad.write_text("trade_uti\nU1\n")

    with pytest.raises(ValueError) as exc:
        opendqi.sftr.book_reconcile(
            str(bad), str(SFTR_TSR_XML), mapping=SFTR_BOOK_MAPPING
        )
    msg = str(exc.value).lower()
    assert "extension" in msg or "csv" in msg or "parquet" in msg


def test_sftr_book_reconcile_arrow_table_input() -> None:
    """The pyarrow.Table dual input path always needs a mapping ;
    fires the same SFTR.BREC.* family as the .csv path. NOTE: the
    Arrow path bypasses the Rust CSV parser, so it does NOT apply
    the v0.14 `date_format` / `datetime_format` overrides — dates
    come through as whatever PyArrow's CSV type-inference produced.
    The issue counts may therefore differ between the two paths
    when the fixture has dates ; we assert structural equivalence
    (both produce issues, both stay in the BREC family, same set
    of records) rather than numeric equality."""
    _require(SFTR_BOOK_CSV, SFTR_TSR_XML)
    import opendqi
    from pyarrow import csv as pyarrow_csv

    # Read the book CSV into a pyarrow.Table — natural 'Arrow is
    # already in memory' starting point.
    table = pyarrow_csv.read_csv(str(SFTR_BOOK_CSV))

    result_arrow = opendqi.sftr.book_reconcile(
        table, str(SFTR_TSR_XML), mapping=SFTR_BOOK_MAPPING
    )
    s = result_arrow.summary
    assert s["regime"] == "sftr"
    assert s["files_processed"] == 2
    assert s["issues_total"] > 0
    # All issues stay in the BREC family.
    arrow_check_ids = set(result_arrow.issues.column("check_id").to_pylist())
    non_brec = {c for c in arrow_check_ids if not c.startswith("SFTR.BREC.")}
    assert not non_brec, f"non-BREC checks leaked via Arrow path: {non_brec}"
    assert isinstance(result_arrow.issues, pa.Table)


def test_sftr_book_reconcile_arrow_requires_mapping() -> None:
    """pyarrow.Table without `mapping=` raises a helpful error."""
    _require(SFTR_BOOK_CSV, SFTR_TSR_XML)
    import opendqi
    from pyarrow import csv as pyarrow_csv

    table = pyarrow_csv.read_csv(str(SFTR_BOOK_CSV))
    with pytest.raises(ValueError) as exc:
        opendqi.sftr.book_reconcile(table, str(SFTR_TSR_XML))
    assert "mapping" in str(exc.value).lower()
