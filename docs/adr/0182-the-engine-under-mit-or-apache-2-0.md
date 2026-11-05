# 0182. The engine under MIT OR Apache-2.0

- Status: Accepted; amends [0006](0006-dependency-policy.md) (the license that dependencies must
  fit) and [0181](0181-uwuis-scope.md) (the engine leaves MPL-2.0, as uwui does)
- Date: 2026-11-05

## Context

The engine was under MPL-2.0, a copyleft at the level of files: whoever distributes a modified file
of the engine publishes that file's source. Games built with it could take any license.
[ADR-0181](0181-uwuis-scope.md) put uwui, which the engine depends on, under MIT OR Apache-2.0, the
license of the Rust ecosystem, and kept the engine under MPL-2.0.

A game ported to a console changes engine files with code that the console's maker keeps under a
non-disclosure agreement, which MPL-2.0 would require it to publish. Engines of the field are under
permissive licenses for that reason among others: Godot under MIT, Bevy under MIT OR Apache-2.0.
Every commit of the engine is under the owner's identity alone.

## Decision

**SlimeEngine is licensed MIT OR Apache-2.0**, at the user's choice, as uwui is: `LICENSE-MIT` and
`LICENSE-APACHE` at the repository's root, `license = "MIT OR Apache-2.0"` in the workspace's
`Cargo.toml`, and contributions accepted under both. Releases tagged before stay under MPL-2.0.
Dependencies under MPL-2.0 stay allowed: their copyleft binds their own files, not the engine's.

## Alternatives considered

- **MPL-2.0 kept**: improvements of the engine's files stay open, but a port to a console cannot
  change them without publishing what it may not.
- **Apache-2.0 alone**: a grant of patents, but programs under GPL-2.0 left out, and the engine
  apart from uwui and the ecosystem.

## Consequences

- The engine and uwui share a license; code moves between them without a question of license.
- Others may ship changes of the engine without their source.
