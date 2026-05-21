"""
OpenDQI Polars interop (v0.14.0).

Native LazyFrame fast path: pushes column selection into Polars
**before** materializing the frame, then converts only the
selected columns to Arrow before handing off to
``opendqi.{emir,sftr}.scan_table``. Zero-copy on most dtypes
between Polars and Arrow.

For wide LazyFrames (many columns, only a few mapped to canonical
EMIR/SFTR fields), this is a real speed win versus collecting
the full frame and then calling ``scan_table`` on the
materialized pyarrow.Table.

Polars is NOT declared as a runtime dependency of the ``opendqi``
wheel — duck-typed import inside the function body. Users opt in
via:

    pip install opendqi[polars]
"""
from __future__ import annotations

from typing import TYPE_CHECKING, Mapping

if TYPE_CHECKING:  # pragma: no cover
    from polars import LazyFrame as PolarsLazyFrame

import opendqi


def scan_lazyframe(
    lf: "PolarsLazyFrame",
    *,
    regime: str = "emir",
    mapping: Mapping[str, str],
    normalize: bool = False,
):
    """Run an opendqi scan on a Polars LazyFrame, with push-down
    column selection.

    Parameters
    ----------
    lf :
        A ``polars.LazyFrame`` whose schema contains at least the
        columns listed in ``mapping.values()``.
    regime :
        ``"emir"`` or ``"sftr"``. Selects which check pack runs
        (89 EMIR or 44 SFTR single-batch checks).
    mapping :
        ``canonical_field`` → ``polars_column_name``. Same direction
        as :func:`opendqi.emir.scan_table` and the CLI's
        ``--mapping`` YAML. Only the columns whose names appear in
        ``mapping.values()`` are materialized (push-down select).
    normalize :
        Passthrough to ``scan_table`` — when True, the
        canonical-model Arrow batch is also exposed on
        ``result.normalized``.

    Returns
    -------
    opendqi.PyScanResult
        Same shape as the result of
        :func:`opendqi.emir.scan_table`.

    Notes
    -----
    The column push-down is the speed-up: ``lf.select(needed)``
    creates a new LazyFrame with only the mapped columns, then
    ``.collect()`` materializes only those. Polars' ``to_arrow()``
    is zero-copy on most dtypes (Utf8, numeric, datetime), so
    handing off to ``scan_table`` is cheap.

    For a LazyFrame already in memory (i.e. you've already
    ``.collect()``ed), use ``opendqi.{emir,sftr}.scan_table(
    df.to_arrow(), mapping=...)`` directly.
    """
    if regime not in ("emir", "sftr"):
        raise ValueError(
            f"unknown regime: {regime!r} (expected 'emir' or 'sftr')"
        )

    # Push-down column selection: keep only the columns the
    # mapping references. For a 100-col LazyFrame with 5 mapped
    # columns, this is a 20× materialization speedup.
    needed = sorted(set(mapping.values()))
    df = lf.select(needed).collect()

    # Polars → Arrow: zero-copy on most dtype combos.
    table = df.to_arrow()

    if regime == "emir":
        return opendqi.emir.scan_table(
            table, dict(mapping), normalize=normalize
        )
    return opendqi.sftr.scan_table(
        table, dict(mapping), normalize=normalize
    )


__all__ = ["scan_lazyframe"]
