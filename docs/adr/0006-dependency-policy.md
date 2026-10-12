# 0006. Dependency policy

- Status: Accepted; amended by [0023](0023-dependency-checks-for-the-supported-targets.md) (targets checked)
- Date: 2026-10-12

## Context

The roadmap's principle 7 says existing crates are used wherever they are not the point of the
engine, wrapped at the boundary so they can be replaced. Every dependency still has a cost: code
the project does not control, compile time, security exposure, and license obligations. The
engine is distributed under MPL-2.0 and games built with it may be proprietary, so a dependency
whose license would impose terms on a game (GPL, LGPL, AGPL) cannot enter the engine.

Reviewers cannot check licenses and security advisories of a whole dependency tree by hand, and
a decision about a dependency is lost if it is not written next to the code.

## Decision

**Admission.** A third-party crate is added only after the owner agrees, and only if:

- it is not the point of the engine (what makes SlimeEngine valuable is written here);
- it is maintained and widely used, or small enough to maintain ourselves if needed;
- its license is in the allowed list below, and so are the licenses of what it pulls in;
- it supports the project's Rust version (ADR-0003);
- its types do not appear in the engine's public API, unless that is the explicit purpose of
  the dependency (for example `raw-window-handle`, whose traits are the standard way to pass a
  window to a graphics library);
- its compile-time cost is acceptable for what it brings.

**Allowed licenses.** MIT, Apache-2.0 (also with the LLVM exception), BSD-2-Clause,
BSD-3-Clause, ISC, Zlib, Unicode-3.0, BSL-1.0, CC0-1.0 and MPL-2.0. Copyleft licenses that reach
beyond the file (GPL, LGPL, AGPL) are refused, as are licenses that are not OSI-approved.

**Record.** Every dependency is listed in [`docs/dependencies.md`](../dependencies.md) with the
crates that use it, why it is needed, the alternative that was considered, and its license.
Dependencies that shape the architecture (a windowing library, a logging framework) also get
their own ADR. Versions are declared once in `[workspace.dependencies]` (ADR-0004).

**Enforcement.** [`deny.toml`](../../deny.toml) configures
[cargo-deny](https://github.com/EmbarkStudios/cargo-deny), which runs in the checks and fails
when:

- a crate's license is not in the allowed list;
- a dependency is affected by a RustSec security advisory (vulnerability, unsound or unmaintained
  crate), unless `deny.toml` ignores that advisory with a written reason;
- a dependency comes from a registry other than crates.io or from a git repository;
- a dependency is declared with a wildcard version.

Several versions of the same crate in the tree produce a warning, not a failure.

## Alternatives considered

- **cargo-vet** (recorded audits of every dependency's source). Thorough, but its audit
  workload is out of proportion with a project maintained by one owner.
- **cargo-audit for advisories plus a separate license tool.** Two tools for what cargo-deny
  does alone.
- **Review by hand only.** Does not scale to transitive dependencies and is forgotten under
  time pressure.

## Consequences

- Adding a dependency takes a conversation and a line in the register, which slows down casual
  additions on purpose.
- A new advisory published against a dependency can turn the checks red without any change in
  the repository. That is intended: it is the moment to upgrade, replace or explicitly ignore it.
- The checks need network access to download the advisory database.
- Changing the allowed licenses is an edit to `deny.toml` and to this policy, through a new ADR.
