# Dependencies

Every third-party dependency of the project, with the reason it is used. The policy that admits
dependencies is [ADR-0006](adr/0006-dependency-policy.md); [`deny.toml`](../deny.toml) enforces its
license, advisory and source rules. Update this file in the same change that adds, removes or
repurposes a dependency.

## Crates

None. The workspace has no third-party crate dependency yet.

## Development tools

Provided by the Nix development shell ([`flake.nix`](../flake.nix)); contributors without Nix
install them as described in [`CONTRIBUTING.md`](../CONTRIBUTING.md#development-environment).

| Tool | Why | Alternative considered | License |
|---|---|---|---|
| Rust toolchain (rustc, cargo, clippy, rustfmt, rust-analyzer) | Builds and checks the code; version pinned in `rust-toolchain.toml` ([ADR-0003](adr/0003-toolchain-edition-and-msrv.md)) | — | MIT OR Apache-2.0 |
| [rust-overlay](https://github.com/oxalica/rust-overlay) (flake input) | Turns `rust-toolchain.toml` into a Nix package, so Nix and rustup users get the same toolchain | The toolchain packaged by nixpkgs, whose version the project does not choose | MIT |
| [nixpkgs](https://github.com/NixOS/nixpkgs) (flake input) | Provides every other tool of the development shell | — | MIT |
| [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) | Checks dependency licenses, security advisories and sources ([ADR-0006](adr/0006-dependency-policy.md)) | cargo-audit plus a separate license tool | MIT OR Apache-2.0 |
| [lychee](https://github.com/lycheeverse/lychee) | Checks links and anchors between Markdown files ([ADR-0007](adr/0007-documentation-tooling-and-ci.md)) | markdown-link-check, which needs Node.js | Apache-2.0 OR MIT |

## CI actions

Used by [`.github/workflows/ci.yml`](../.github/workflows/ci.yml), pinned to commit SHAs.

| Action | Why | License |
|---|---|---|
| [actions/checkout](https://github.com/actions/checkout) | Checks out the repository | MIT |
| [cachix/install-nix-action](https://github.com/cachix/install-nix-action) | Installs Nix with flakes enabled, so CI runs the same development shell as contributors | Apache-2.0 |
