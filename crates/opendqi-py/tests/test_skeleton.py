"""
P1 skeleton smoke-test — assert `import opendqi` works and the
package exposes the expected version + submodules.

Run via `pytest crates/opendqi-py/tests/` from inside a venv where
`maturin develop` has been executed.
"""
from __future__ import annotations


def test_import_opendqi() -> None:
    import opendqi
    assert opendqi is not None


def test_version_string() -> None:
    import opendqi
    # P1: version is the one declared in Cargo.toml / pyproject.toml.
    # Bumped together at release time.
    assert isinstance(opendqi.__version__, str)
    # Match the Cargo workspace versioning scheme (semver).
    parts = opendqi.__version__.split(".")
    assert len(parts) >= 2, f"unexpected version shape: {opendqi.__version__!r}"
    assert opendqi.__version__.startswith("0.12."), (
        f"P1 expected 0.12.x, got {opendqi.__version__!r}"
    )


def test_submodules_present() -> None:
    """Both regime submodules are registered and importable."""
    import opendqi
    # Direct attribute access (submodules registered via PyO3
    # `add_submodule`).
    assert hasattr(opendqi, "emir"), "opendqi.emir submodule missing"
    assert hasattr(opendqi, "sftr"), "opendqi.sftr submodule missing"
    assert opendqi.emir.__doc__ is not None
    assert opendqi.sftr.__doc__ is not None


def test_submodules_carry_scan_parquet() -> None:
    """P2: each regime submodule exposes `scan_parquet(path)`."""
    import opendqi
    assert callable(opendqi.emir.scan_parquet)
    assert callable(opendqi.sftr.scan_parquet)
