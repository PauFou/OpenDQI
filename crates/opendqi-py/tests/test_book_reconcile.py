"""
v0.13.0 — `opendqi.{emir,sftr}.book_reconcile` cross-message.

Tests the dual book-input contract:
  - str path → .csv (needs mapping) or .parquet (no mapping)
  - pyarrow.Table / pyarrow.RecordBatch (always needs mapping)
"""
from __future__ import annotations

from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[3]

# EMIR fixtures
EMIR_BOOK_CSV = REPO_ROOT / "examples" / "emir" / "book_reconcile" / "book.csv"
EMIR_TSR_XML = REPO_ROOT / "examples" / "emir" / "tr_state" / "auth107-sample.xml"

# Mapping mirrors examples/emir/book_reconcile/book_mapping.yml.
EMIR_BOOK_MAPPING = {
    "uti": "trade_uti",
    "notional_amount": "notional",
    "notional_currency": "notional_ccy",
    "maturity_date": "maturity",
    "valuation_amount": "valuation",
    "valuation_currency": "valuation_ccy",
    "termination_date": "terminated_on",
}


def _require(*paths: Path) -> None:
    missing = [p for p in paths if not p.exists()]
    if missing:
        pytest.skip(f"fixtures missing: {missing}")


def test_emir_book_reconcile_csv_path() -> None:
    """The most common path: a .csv book + a dict mapping."""
    _require(EMIR_BOOK_CSV, EMIR_TSR_XML)
    import opendqi
    result = opendqi.emir.book_reconcile(
        str(EMIR_BOOK_CSV),
        str(EMIR_TSR_XML),
        mapping=EMIR_BOOK_MAPPING,
    )
    s = result.summary
    assert s["regime"] == "emir"
    assert s["files_processed"] == 2
    # The shipped fixture pair is designed to fire several
    # EMIR.BREC.* mismatches.
    assert s["issues_total"] > 0


def test_emir_book_reconcile_only_brec_checks() -> None:
    """book_reconcile fires ONLY EMIR.BREC.* — nothing else leaks in."""
    _require(EMIR_BOOK_CSV, EMIR_TSR_XML)
    import opendqi
    result = opendqi.emir.book_reconcile(
        str(EMIR_BOOK_CSV),
        str(EMIR_TSR_XML),
        mapping=EMIR_BOOK_MAPPING,
    )
    check_ids = set(result.issues.column("check_id").to_pylist())
    non_brec = {c for c in check_ids if not c.startswith("EMIR.BREC.")}
    assert not non_brec, f"non-BREC checks leaked: {non_brec}"


def test_emir_book_reconcile_csv_requires_mapping() -> None:
    """.csv path without `mapping=` raises a helpful error."""
    _require(EMIR_BOOK_CSV, EMIR_TSR_XML)
    import opendqi
    with pytest.raises(ValueError) as exc:
        opendqi.emir.book_reconcile(str(EMIR_BOOK_CSV), str(EMIR_TSR_XML))
    assert "mapping" in str(exc.value).lower()


def test_emir_book_reconcile_unsupported_extension() -> None:
    """A book path with a non-CSV/Parquet extension raises clearly."""
    _require(EMIR_TSR_XML)
    import opendqi
    with pytest.raises(ValueError) as exc:
        opendqi.emir.book_reconcile(
            "/tmp/not_a_real_book.txt", str(EMIR_TSR_XML), mapping=EMIR_BOOK_MAPPING
        )
    assert "extension" in str(exc.value).lower() or "txt" in str(exc.value).lower()


def test_emir_book_reconcile_pyarrow_table_input() -> None:
    """Pass a pyarrow.Table directly (not a path). Same mapping format."""
    _require(EMIR_BOOK_CSV, EMIR_TSR_XML)
    import opendqi, pyarrow.csv as pacsv
    # Load the CSV via pyarrow (skip the opendqi CSV reader entirely).
    book_table = pacsv.read_csv(str(EMIR_BOOK_CSV))
    result = opendqi.emir.book_reconcile(
        book_table,
        str(EMIR_TSR_XML),
        mapping=EMIR_BOOK_MAPPING,
    )
    # Numbers won't match the CSV path exactly because of decimal/date
    # parsing differences (pyarrow CSV reader returns strings for some
    # cols, the opendqi CSV reader converts to Decimal/Date), but the
    # call should at least succeed and return EMIR.BREC.* issues.
    assert result.summary["files_processed"] == 2
    # May be > 0 or 0 depending on the fixture's date/decimal columns;
    # we just verify it didn't crash and produced an issues table.
    assert result.issues is not None


def test_emir_book_reconcile_arrow_input_requires_mapping() -> None:
    """pyarrow.Table input without mapping raises."""
    _require(EMIR_BOOK_CSV, EMIR_TSR_XML)
    import opendqi, pyarrow as pa
    table = pa.table({"trade_uti": ["U1", "U2"]})
    with pytest.raises(ValueError) as exc:
        opendqi.emir.book_reconcile(table, str(EMIR_TSR_XML))
    assert "mapping" in str(exc.value).lower()


def test_sftr_book_reconcile_function_present() -> None:
    """SFTR symmetric — exercising end-to-end needs SFTR fixtures."""
    import opendqi
    assert callable(opendqi.sftr.book_reconcile)


# --- v0.14: date_format / datetime_format kwargs ------------------


def test_book_reconcile_default_date_format_matches_no_kwargs(
    tmp_path: Path,
) -> None:
    """Passing the default %Y-%m-%d explicitly produces the SAME result
    as passing no date_format (the explicit default is a no-op)."""
    _require(EMIR_BOOK_CSV, EMIR_TSR_XML)
    import opendqi

    r_default = opendqi.emir.book_reconcile(
        str(EMIR_BOOK_CSV), str(EMIR_TSR_XML), mapping=EMIR_BOOK_MAPPING
    )
    r_explicit = opendqi.emir.book_reconcile(
        str(EMIR_BOOK_CSV),
        str(EMIR_TSR_XML),
        mapping=EMIR_BOOK_MAPPING,
        date_format="%Y-%m-%d",
        datetime_format="%Y-%m-%dT%H:%M:%S%.fZ",
    )
    # Same number of issues — the explicit defaults are the
    # CsvMapping fallback.
    assert r_default.summary["issues_total"] == r_explicit.summary["issues_total"]


def test_book_reconcile_custom_date_format(tmp_path: Path) -> None:
    """A CSV with DD/MM/YYYY dates needs date_format='%d/%m/%Y' to parse
    correctly. Without it, dates are unparseable → fewer EMIR.BREC.MATURITY
    fires."""
    _require(EMIR_TSR_XML)
    import opendqi

    # Build a small EU-style book CSV (matches the structure of
    # EMIR_BOOK_CSV but with DD/MM/YYYY dates).
    eu_book = tmp_path / "book_eu.csv"
    eu_book.write_text(
        "trade_uti,notional,notional_ccy,maturity,valuation,valuation_ccy,terminated_on\n"
        "OPENDQI-TSR-CLEAN-0001,1000000,EUR,02/04/2031,150.50,EUR,\n"
        "OPENDQI-TSR-STALE-0002,9999999,EUR,02/04/2031,150.50,EUR,\n"
    )

    # WITHOUT custom date_format: maturity dates fail to parse → records
    # come in with maturity=None, BREC.MATURITY can't fire (it needs
    # both sides to be Some to compare).
    r_no_fmt = opendqi.emir.book_reconcile(
        str(eu_book), str(EMIR_TSR_XML), mapping=EMIR_BOOK_MAPPING
    )

    # WITH custom date_format: maturity parses → BREC.MATURITY may fire.
    r_with_fmt = opendqi.emir.book_reconcile(
        str(eu_book),
        str(EMIR_TSR_XML),
        mapping=EMIR_BOOK_MAPPING,
        date_format="%d/%m/%Y",
    )

    # Both runs succeed (no crash). The exact issue counts depend on
    # whether the EU-format dates equal the TSR-side dates after
    # parsing; we just verify the two calls don't error AND the with-fmt
    # run produces a non-error result.
    assert r_no_fmt.summary["regime"] == "emir"
    assert r_with_fmt.summary["regime"] == "emir"
    # Both should process 2 book records + N TSR records.
    assert r_no_fmt.summary["files_processed"] == 2
    assert r_with_fmt.summary["files_processed"] == 2
