# uwui-blank-winit

A *blank window* on winit and wgpu: a program that clears its window to one colour and presents
it, again at each input of the person, and draws nothing at rest. It is the reference that uwui's
platform layer and desktop host are measured beside
([ADR-0279](../../docs/adr/0279-one-windowing-layer-a-seam-in-uwuis-host-now-the-engine-moved-onto-it-at-a-port.md),
[ADR-0307](../../docs/adr/0307-m6d7a2c-cut-in-two-the-host-measured-first-the-blank-window-on-winit-in-a-package-of-its-own.md)).
Unpublished, under MIT OR Apache-2.0; it leaves with uwui at M6d.11.

## Role

- Opens a window through winit, and a device as `uwui-desktop` opens its own: the adapter that
  uses the least power, unless `WGPU_POWER_PREF` names another; the adapter's limits; wgpu's hint
  `MemoryUsage`; an sRGB image, Mailbox where the surface offers it, else Fifo.
- Draws as winit asks of a program: a frame on each `RedrawRequested`, asked by `request_redraw`
  at each input, `pre_present_notify` before each present, which paces the frames on the
  compositor's frame callback on Wayland; a frame that finds no image asked again 100 ms later, as
  the host does.
- Writes uwui's desktop host's trace where `UWUI_TRACE` names a file, with a writer of its own:
  the same events in the same format (`uwui_desktop::TRACE_ENV_VAR`), an input's time its
  reception, since winit gives none.
- `scripts/host-cost.py` runs it beside `uwui-desktop`'s examples `blank` and `application`, in
  one compositor, and compares their start, memory, binary, rest and input to present.

A lost device or surface ends its run. It takes no termination signal: SIGTERM ends it as the
system does. Its trace's times follow Rust's `Instant` from one reading of the system's time.

## Entry points

`src/main.rs`, the whole program: `cargo run -p uwui-blank-winit`. It needs a Wayland compositor
or an X server, and a GPU with Vulkan.

## Dependencies

winit, the windowing layer it stands for; wgpu, which clears and presents; `pollster`, which waits
for wgpu's requests.

## Must not

- Grow beyond a blank window: anything it adds is counted against uwui's own cost.
- Depend on a crate of uwui's or of the engine's: it stays the measure of winit and wgpu alone.
