# 0005. Lints and `unsafe` policy

- Status: Accepted
- Date: 2026-10-12

## Context

The project's rules require rustdoc on every public item, including its errors and panics, and a
`// SAFETY:` comment on every `unsafe` block. Rules that a tool can check should be checked by a
tool rather than by reviewers. At the same time, work in progress must compile while it is being
written, and a noisy lint set teaches people to silence lints.

Rust's memory safety guarantees hold only outside `unsafe` code. An engine will eventually need
some (operating system interfaces, performance work), but each use must be rare, justified and
easy to find.

## Decision

Lints are declared once in the workspace `Cargo.toml` (`[workspace.lints]`) and every crate
inherits them with `[lints] workspace = true`. No crate declares its own lint table.

- `missing_docs` (rustc): warn. Every public item is documented.
- `unsafe_code` (rustc): deny. See the `unsafe` policy below.
- `clippy::missing_errors_doc` and `clippy::missing_panics_doc`: warn. Public functions that
  return `Result` document their errors in an `# Errors` section, and those that can panic
  document it in a `# Panics` section.
- `clippy::undocumented_unsafe_blocks`: warn. Every `unsafe` block is preceded by a
  `// SAFETY:` comment.
- `clippy::allow_attributes_without_reason`: warn. A lint is silenced with
  `#[expect(lint, reason = "...")]`, which also fails once the lint no longer fires, rather than
  with a bare `#[allow]`.
- Clippy's default lint groups apply. The `pedantic` group is not enabled as a whole; individual
  lints are added to `[workspace.lints]` when a real problem shows they are needed, with a
  comment saying why.
- Warnings are warnings while developing, and errors in the checks: clippy runs with
  `-D warnings` and rustdoc with `RUSTDOCFLAGS="-D warnings"`, so rustdoc's own default warnings
  (broken intra-doc links, for example) fail the checks too.

`unsafe` policy:

- `unsafe` code is allowed only to call foreign or operating system interfaces, or to reach a
  performance target that a measurement shows safe code cannot meet.
- It is enabled on the smallest possible item with
  `#[expect(unsafe_code, reason = "...")]`, never for a whole crate.
- Every `unsafe` block has a `// SAFETY:` comment explaining why it is sound, and every `unsafe`
  function has a `# Safety` section stating what callers must guarantee.
- Introducing `unsafe` into a crate that has none is an architecture decision: ask the owner
  first.
- `docs/ARCHITECTURE.md` lists the crates that contain `unsafe` code. When such code can run
  under Miri (Rust's interpreter that detects undefined behaviour), its tests run under Miri in
  CI.

## Alternatives considered

- **`#![forbid(unsafe_code)]`.** It cannot be lifted locally, so the first legitimate use would
  force a policy change instead of a reviewed exception.
- **No `unsafe` lint, review only.** Reviewers miss things; a compiler error does not.
- **The whole `clippy::pedantic` group.** Many of its lints are matters of taste and produce
  noise that leads to blanket `allow`s.
- **Lint levels in each crate's source (`#![warn(...)]`).** They drift apart between crates;
  the workspace table keeps one policy.

## Consequences

- Undocumented public items, undocumented `unsafe` and unexplained lint exceptions cannot reach
  the main branch without failing the checks.
- Writing an `unsafe` block requires an explicit, local, justified opt-in, which makes every use
  easy to find with a search for `expect(unsafe_code`.
- Changing a lint level is a one-line change in `Cargo.toml`; removing one of the lints above
  needs a new ADR.
