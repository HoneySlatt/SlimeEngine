# 0007. Documentation tooling and continuous integration

- Status: Accepted
- Date: 2026-10-12

## Context

Documentation is part of every deliverable, held to the same standard as the code, and a newcomer
must be able to understand the whole repository from it. Documentation that is not checked drifts:
links break, code examples stop compiling, and the API reference fills with warnings that nobody
reads.

Checks that only run in CI are discovered late, and checks that run differently locally and in CI
produce failures nobody can reproduce. The project is hosted on GitHub.

## Decision

**Documentation tooling.**

- API documentation is rustdoc. Every public item is documented (ADR-0005); code examples in
  rustdoc compile and run as doc tests.
- Project documentation is Markdown in the repository: `README.md`, `CONTRIBUTING.md`, `docs/`.
  There is no generated book (mdBook) until there is a user guide to publish.
- Each crate's `README.md` is also its rustdoc front page, included with
  `#![doc = include_str!("../README.md")]`. There is one text to maintain, and its Rust code
  blocks run as doc tests. Because the same text is rendered by GitHub and by rustdoc, crate
  READMEs contain no relative links (a relative link cannot be right in both places): they name
  paths in code spans instead.
- Links between Markdown files, including links to headings, are checked by
  [lychee](https://github.com/lycheeverse/lychee) in offline mode. External links are not
  checked automatically, because they fail for reasons unrelated to the change being checked.

**Checks and CI.**

- [`scripts/check.sh`](../../scripts/check.sh) runs every automated check of the definition of
  done: rustfmt, clippy and rustdoc with warnings as errors, build and tests of all targets,
  cargo-deny (ADR-0006), and the link check. It runs inside the Nix development shell:
  `nix develop -c scripts/check.sh`.
- CI is a GitHub Actions workflow ([`.github/workflows/ci.yml`](../../.github/workflows/ci.yml))
  that installs Nix and runs that same script, on every push to `main` and every pull request.
  CI therefore uses the toolchain and tool versions pinned by `rust-toolchain.toml` and
  `flake.lock`.
- Third-party actions are pinned to a full commit SHA, with the corresponding release in a
  comment, so that a moved tag cannot change what runs in CI. The workflow only gets read access
  to the repository.
- Running a stage's sample is part of the checks once it exists; headless samples run in CI.

## Alternatives considered

- **rustdoc plus mdBook now.** A book is worth its upkeep when there is a guide for engine users;
  today there is none.
- **Separate crate READMEs and crate-level rustdoc.** Two texts describing the same crate drift
  apart.
- **A CI workflow that installs Rust with rustup and runs the commands itself.** Faster to set
  up, but the toolchain and tools would differ from the ones contributors use, and the list of
  commands would live in two places.
- **Checking external links in CI.** Network flakiness would make unrelated changes fail.

## Consequences

- One command reproduces CI locally, and adding a check means editing one script.
- CI depends on Nix being installable on GitHub's runners and on the Nix binary cache; the first
  run of a new `flake.lock` downloads the toolchain.
- Broken external links can go unnoticed; they are fixed when someone finds them.
- Introducing mdBook later adds a tool and a directory without changing anything above.
