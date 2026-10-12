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
| [`headless`](headless.rs) | An application without a window: frames back to back with simulated time, exit at a frame limit, profiling spans (`SLIME_LOG=trace`) | `--frames <count>` (default 600) |
| [`window`](window.rs) | A window in which the GPU draws the samples' triangle, which follows resizes; its events logged (window, keys, mouse; cursor motion with `SLIME_LOG=debug`); Escape, closing it or Ctrl+C quits cleanly. The sample of Stage 2 | `--frames <count>` closes it after that many frames; `--headless` runs the same program without a window; `--present-mode mailbox|fifo` chooses how images are shown (default `mailbox`) |
| [`offscreen`](offscreen.rs) | Rendering without a window: the GPU draws a triangle into an image, which is read back and written to a PNG file; works on software drivers (lavapipe, WARP) | `--output <file>` (default `triangle.png`) |

On Windows, the same commands work with the toolchain described in
[`CONTRIBUTING.md`](../CONTRIBUTING.md#on-windows). Each CI run on GitHub also publishes
`hello.exe`, `headless.exe` and `window.exe` built for Windows, as the artifact
`slime-examples-windows`, to run on a Windows machine without a Rust toolchain.

The log level of every example is set by the `SLIME_LOG` environment variable, for example
`SLIME_LOG=debug cargo run --example hello`.

## Shared code and tests

- [`common/`](common) holds code used by several examples and tests, each file included where it
  is needed with `#[path = "common/<file>.rs"] mod <file>;`: the triangle that `window`,
  `offscreen` and the image test draw, and its shader; PNG writing; error messages with their
  causes.
- [`tests/offscreen.rs`](tests/offscreen.rs) renders the same triangle and compares it with the
  reference image [`tests/reference/triangle.png`](tests/reference/triangle.png), within a
  tolerance ([ADR-0018](../docs/adr/0018-reference-image-tests.md)). Its module comment explains
  how to render the reference again after a deliberate change.

To add an example, put its source in this directory, declare it as an `[[example]]` in
[`Cargo.toml`](Cargo.toml), give it a module comment saying what it shows and how to run it, and
add it to the table above.
