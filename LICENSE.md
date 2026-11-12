# SlimeEngine's licence: interim notice

Copyright © 2026 HoneySlatt. All rights reserved, except as this notice says.

SlimeEngine is to be licensed under a licence of its own: free to use until a game has earned
1 million US dollars of gross revenue, with a royalty above it
([ADR-0214](docs/adr/0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md)). Its text is
not written yet. Until it is published here, the source in this repository is visible so that it can
be read, and no licence is granted to use, copy, modify or distribute it, beyond what the law allows
without one and what GitHub's terms of service let its users do on GitHub.

What this notice does not cover:

- **uwui**, the user-interface framework in `crates/uwui`, `crates/uwui-agent`,
  `crates/uwui-render`, `crates/uwui-kdl` and `crates/uwui-macros`, and `crates/slime-color` and `crates/slime-input`, which move to
  uwui's repository, are licensed under either of the MIT license or the Apache License 2.0, at
  your choice, as the `LICENSE-MIT` and `LICENSE-APACHE` files in each of these crates say. uwui
  still depends on four crates of the engine, `slime-reflect`, `slime-serialize`, `slime-gfx` and
  `slime-render`, which this notice covers, until Stage 6d removes those dependencies.
- **The examples**, in `examples/`, are licensed under MIT-0, as `examples/LICENSE-MIT-0` says: code
  made to be copied into games, without conditions
  ([ADR-0219](docs/adr/0219-test-games-in-place-of-hunna-wood.md)). Their assets keep the licences
  that `examples/assets/` records.
- **What was published before this notice keeps its licence.** The commits published from
  2026-11-05 up to the commit that adds this notice are under MIT OR Apache-2.0; the releases up to
  `v0.6.0`, and the commits before 2026-11-05, under the Mozilla Public License 2.0. The history of
  the repository holds those terms.
- **The work of others** keeps its own licence: the dependencies, listed in
  [`docs/dependencies.md`](docs/dependencies.md); the fonts and icons, beside their files; the
  sample game's assets, in [`games/hunnawood/assets/sources.tsv`](games/hunnawood/assets/sources.tsv).
- **The logos** have terms of their own: [`docs/logo/README.md`](docs/logo/README.md) for the
  engine's, [`crates/uwui/logo/README.md`](crates/uwui/logo/README.md) for uwui's.

Contributions of code are not accepted before 0.14; from then, under a contributor licence
agreement ([`CONTRIBUTING.md`](CONTRIBUTING.md#license-of-contributions)).
