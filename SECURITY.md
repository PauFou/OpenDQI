# Security Policy

## Supported versions

OpenDQI is in early development. Only the latest commit on `main` receives security fixes.

## Reporting a vulnerability

Please report security issues privately by opening a GitHub security advisory on this repository, or by emailing the maintainers directly. Do **not** open a public issue for security reports.

When reporting, include:

- A description of the issue.
- Steps to reproduce.
- The version or commit hash affected.
- Any known mitigations.

We aim to acknowledge reports within 5 business days.

## Scope

OpenDQI is a local-first tool. It does not, by default, transmit data over the network. Security reports should focus on:

- arbitrary file read / write outside the user-specified `--out` directory
- denial of service from malformed input files
- memory safety issues in the parsing layers
- injection of executable content into generated HTML reports

Out of scope:

- denial of service from intentionally pathological inputs (e.g. multi-gigabyte CSVs) — performance is a goal, not a security boundary.
- vulnerabilities in third-party dependencies that have no exploitable path inside OpenDQI.

## Disclaimer

OpenDQI is not a regulatory certification tool. Users remain responsible for the confidentiality and integrity of any data they process with it.
