# Examples

Runnable programs that each demonstrate one engine feature. They belong to the `slime-examples`
package, which has no other purpose and is never published. Run them from the repository root,
inside `nix develop`:

```sh
cargo run --example <name> [-- <arguments>]
```

| Example | Shows | Arguments |
|---|---|---|
| [`hello`](hello.rs) | Diagnostics: logging through `tracing`, the `SLIME_LOG` filter, panic reporting | `--panic` panics on purpose |
| [`headless`](headless.rs) | A game's rules without a window, a `Simulation`: frames back to back on simulated time, profiling spans | `--frames <count>`, 600 by default |
| [`text`](text.rs) | The interface's text without a GPU: a sentence in each writing system laid out, rasterized and written to a PNG file ([ADR-0136](../docs/adr/0136-the-interfaces-text.md)) | `--output <file.png>`; `--scale <factor>` |
| [`window`](window.rs) | A window in which the GPU draws a triangle; its events logged; the cursor held while the right button is ([ADR-0157](../docs/adr/0157-dialogs-the-cursor-held-and-the-gpus-identity.md)). The sample of Stage 2 | `--frames <count>`; `--headless`; `--present-mode mailbox\|fifo`; `--frame-rate <per second>` |
| [`offscreen`](offscreen.rs) | Rendering without a window: a triangle drawn into an image and written to a PNG file, on software drivers too | `--output <file>` |
| [`scene`](scene.rs) | A cooked scene drawn by the renderer, lit in physical units under a sky with bloom, a camera that orbits it, and a dev overlay of the frame's times and sliders ([ADR-0069](../docs/adr/0069-the-asset-pipeline.md)). The sample of Stages 3a, 3b, 4a and 4b | `--assets <directory>`; `--scene <id>`; `--frames <count>`; `--watch` |
| [`world`](world.rs) | The examples' *yard*, a scene file of prefabs, spawned in a world and drawn from it in two views, picked by a click, edited in the dev overlay, saved and written back when it quits ([ADR-0084](../docs/adr/0084-what-the-world-draws.md)) | `--assets <directory>`; `--frames <count>`; `--headless`; `--watch` |
| [`editor`](editor.rs) | The editor's program on the yard: menus, hierarchy, viewports, components, history, console and Play, in panels the user rearranges; driven by a person or by an agent through the MCP server ([ADR-0155](../docs/adr/0155-the-editors-design.md)) | `--level <file>`, a copy of the yard, since it writes what it edits; `--assets <directory>`; `--journal <file>`; `--discard`; `--headless`; `--frames <count>`, which quits after that many frames, as `scripts/start-times.sh` times its start; `--output <file.png>`; `--scale <factor>` |
| [`interface`](interface.rs) | An interface on `uwui`: a tree of elements laid out and styled by a theme read from files, its spaces named in code by tokens of its own ([ADR-0238](../docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md)), used with the pointer and the keyboard, read by agents and screen readers, drawn through the renderer's canvas ([ADR-0140](../docs/adr/0140-the-tree-of-elements.md)) | `--assets <dir>`; `--theme dark\|light`; `--watch`; `--frames <count>`; `--output <file.png>`; `--scale <factor>`; `--right-to-left`; `--dialog` |
| [`widgets`](widgets.rs) | The widgets of an editor's panel in the editor's look: tabs, sliders, menus, floating panels, rich fields, notifications, the palette of commands and a color well ([ADR-0148](../docs/adr/0148-widgets.md)) | `--frames <count>`; `--output <file.png>`; `--scale <factor>`; `--right-to-left` |

Each example's module comment, at the top of its file, says in full what it shows, what its keys
do and what each argument means.

The examples that run through `slime_app::run` hold the development channel, `slime-app`'s
`development` feature ([ADR-0122](../docs/adr/0122-the-development-channel.md)): started with a
token, as the engine's MCP server ([`crates/slime-mcp`](../crates/slime-mcp/README.md)) starts a
program, they listen for the tools that drive them; started without one, they do not. They hold
the adapter of screen readers too, `slime-app`'s `accessibility` feature
([ADR-0146](../docs/adr/0146-screen-readers.md)): the window of one that has an interface, as
`interface`, `widgets` and `editor` do, and `scene` and `world` through their dev overlay, gives its tree to
Orca on Linux and to Narrator on Windows.

On Windows, the same commands work with the toolchain described in
[`CONTRIBUTING.md`](../CONTRIBUTING.md#on-windows). Each CI run on GitHub also publishes the
examples built for Windows but `text` and `editor`, as the artifact `slime-examples-windows`, with the game
([`games/hunnawood`](../games/hunnawood/README.md)), the cooked data they read, DXC's library
([ADR-0066](../docs/adr/0066-shipping-dxc-beside-the-programs.md)) and the licenses of the fonts
they embed; [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) lists them. They need neither
a Rust toolchain nor the Visual C++ Redistributable. Run them from a terminal: started by a double
click, their console closes with them, and an error at start-up with it.

The `scene`, `world`, `editor` and `interface` examples read cooked data, which the cooker
(`slime-cook`, [`crates/slime-cook`](../crates/slime-cook/README.md)) makes from the sources in
[`assets/`](assets/README.md); `editor` cooks them itself when they are missing. Cook them once, and
again after a source changes:

```sh
cargo run -p slime-cook -- cook examples/assets target/cooked/examples
cargo run --example scene
cargo run --example world
```

The log level of every example is set by the `SLIME_LOG` environment variable, for example
`SLIME_LOG=debug cargo run --example hello`.

## Shared code and tests

- [`common/`](common) holds the code that several examples and tests share, each file included
  where it is needed with `#[path = "common/<file>.rs"] mod <file>;`. Each file's comment says what
  it holds.
- [`assets/`](assets/README.md) holds the examples' sources and their licenses.
- The examples' code is under MIT-0 ([`LICENSE-MIT-0`](LICENSE-MIT-0)), to be copied into games
  without conditions ([ADR-0219](../docs/adr/0219-test-games-in-place-of-hunna-wood.md)).
- The image tests compare a render with a reference image of
  [`tests/reference/`](tests/reference), within a tolerance
  ([ADR-0018](../docs/adr/0018-reference-image-tests.md), amended by
  [ADR-0028](../docs/adr/0028-image-tests-tolerate-a-few-pixels.md),
  [ADR-0030](../docs/adr/0030-image-tests-strict-on-the-reference-driver.md) and
  [ADR-0054](../docs/adr/0054-image-tests-allow-0-2-percent-on-other-drivers.md)). With
  `--nocapture`, they print the difference they measured. Each test file's module comment says what
  it checks and how to render a reference again after a deliberate change.

| Test | What it checks |
|---|---|
| [`tests/offscreen.rs`](tests/offscreen.rs) | The triangle drawn offscreen |
| [`tests/scene.rs`](tests/scene.rs) | The shapes of `common/` at a fixed time |
| [`tests/gltf.rs`](tests/gltf.rs) | The sample scene cooked and drawn from two camera poses; small glTF files built in memory, cooked and drawn; a cooked file moved and renamed |
| [`tests/world.rs`](tests/world.rs) | The sample drawn from a world, with the images of `tests/gltf.rs`; steps, hidden subtrees, a lost device, picking; the yard's prefabs followed when they change |
| [`tests/widgets.rs`](tests/widgets.rs) | The `widgets` example's interface in its themes, densities, directions and layers; every element's style in each theme and density, against the fixture `tests/reference/widgets-styles.txt` (`common/style_fixture.rs`) ([ADR-0229](../docs/adr/0229-styles-typed-as-declarations.md)); at twice the size of text, no label, button, toggle, segment, tab or check box cut by its box or an ancestor, with the statistics, the sheet of shortcuts and a notification open, WCAG 2.2's 1.4.4 ([ADR-0237](../docs/adr/0237-the-users-text-size-and-less-motion-density-and-contrast-by-names.md)) |
| [`tests/editor.rs`](tests/editor.rs) | The `editor` example's window at scales 1 and 2, with its dialog, a selection and its handles, the history, two viewports, and panels moved and floating; every element's style once the yard is open, against the fixture `tests/reference/editor-styles.txt` |

To add an example, put its source in this directory, declare it as an `[[example]]` in
[`Cargo.toml`](Cargo.toml), give it a module comment saying what it shows and how to run it, and
add it to the table above; to publish it for Windows too, add it to the artifact's list in
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml).
