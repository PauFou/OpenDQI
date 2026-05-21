"""OpenDQI — local-first EMIR/SFTR data quality engine.

Python / Arrow bindings to the Rust core. The bulk of the public
API (`scan_parquet`, `scan_table`, `parse_xml`, the new v0.13
multi-file and cross-message workflows, etc.) is implemented in
the compiled Rust extension `_opendqi` and re-exported from this
package's namespace via ``from ._opendqi import *``.

Pure-Python helpers (currently: `opendqi.spark` — experimental)
live as their own modules inside this package.

See https://github.com/PauFou/OpenDQI for documentation, and
``docs/python.md`` in the source tree for the quickstart.
"""
from __future__ import annotations

# Re-export the compiled extension's surface so that
# `import opendqi; opendqi.emir.scan_parquet(...)` keeps working
# identically to v0.12.x (when the compiled module was named
# `opendqi` directly rather than the `opendqi._opendqi` it lives
# at since v0.13.0's maturin python-source refactor).
from ._opendqi import *  # noqa: F401,F403
from ._opendqi import __version__, __doc__  # noqa: F401
from ._opendqi import emir, sftr  # noqa: F401  — explicit submodule re-export

# v0.13.0 — pure-Python experimental Spark interop. Imported
# lazily-friendly (the module itself is just function definitions;
# `pyarrow` and `pyspark` are imported INSIDE the helper, not at
# module-load time, so plain `import opendqi.spark` is cheap even
# without PySpark installed).
from . import spark  # noqa: F401

__all__ = ["__version__", "emir", "sftr", "spark"]
