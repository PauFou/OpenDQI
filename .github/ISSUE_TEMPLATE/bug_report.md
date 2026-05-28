---
name: Bug report
about: Report a defect in the parser, checks, CLI, Python bindings, or docs.
title: '[bug] <one-line summary>'
labels: bug
assignees: PauFou
---

## What happened

<!-- One paragraph describing what went wrong. -->

## What you expected to happen

<!-- One paragraph describing what should have happened instead. -->

## Steps to reproduce

```bash
# Exact commands or Python snippet
opendqi emir scan ./your-fixture.xml --out /tmp/out/
```

If the bug is parser-related, attach a **minimal synthetic
fixture** (no real counterparties, no real UTIs) demonstrating
the issue.

## Environment

- **OpenDQI version**: `opendqi --version` →
- **Install channel**: [ ] PyPI wheel  [ ] CLI installer  [ ] cargo install
- **OS / arch**: e.g. `macOS 14.5 ARM64`, `Ubuntu 22.04 x86_64`
- **Python version** (if Python bindings): `python --version` →
- **Rust toolchain** (if built from source): `rustc --version` →

## Output

<!-- Paste the relevant CLI stderr / Python traceback / report HTML
     snippet. Redact any sensitive data first. -->

```
<paste here>
```

## Additional context

<!-- Anything else useful: related issues, screenshots, recent
     changes that may have introduced the regression. -->
