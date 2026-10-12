# 0019. Default log filter

- Status: Accepted
- Date: 2026-10-12

## Context

[ADR-0008](0008-errors-logging-and-assertions.md) records events at `info` and above when the
`SLIME_LOG` environment variable is unset. Once the engine opened the GPU, that default printed
lines that do not concern the engine at every start, measured with the offscreen sample on Linux:

- In every build, wgpu-hal (wgpu's layer over Vulkan and Direct3D 12) reports at `info` what the
  GPU supports, for example how many cooperative matrix configurations it found.
- In debug builds, where the engine asks wgpu for debug labels, wgpu-hal relays the messages of
  the Vulkan loader and layers, and the loader warns about every installed driver that fails to
  start. On NixOS, Mesa ships Dozen, a Vulkan driver for Windows' WSL, so each start prints two
  lines of `WARN` that no one can act on.

wgpu-hal's own warnings, on the other hand, report real problems (a failed GPU memory allocation,
a failed capture in RenderDoc, a Direct3D 12 runtime falling back to the system's). wgpu-core and
naga, wgpu's shader compiler, write nothing at `info` with the features the engine enables.

## Decision

When `SLIME_LOG` is unset or empty, the filter is
`info,wgpu_hal=warn,wgpu_hal::vulkan::instance=error` (`slime_app::DEFAULT_LOG_FILTER`):

- `info` and above for everything else, the engine and the game included;
- `warn` and above for wgpu-hal;
- `error` only for `wgpu_hal::vulkan::instance`, the module that relays the Vulkan loader's and
  layers' messages. Vulkan validation errors are still recorded.

Any value of `SLIME_LOG` replaces the default entirely; a module without a level of its own then
records `info` and above, as before.

This amends ADR-0008, whose default filter was `info` for every module.

Entries added since, each with the message that justifies it:

- 2026-10-12: `sctk_adwaita::config=off`. The module of winit's client-side decorations that asks
  the desktop settings portal for the theme logs an `error` whenever `dbus-send` returns nothing,
  with a text that speaks of a timeout although none occurred: at every start on systems without a
  session bus or portal (containers, CI runners, minimal compositors). The engine cannot act on it.

## Alternatives considered

- **Keep `info` for everything.** Every run starts with lines about drivers that the user cannot
  act on, and developers learn to skip warnings.
- **Turn off wgpu's debug labels to silence the Vulkan loader.** The labels are what GPU capture
  tools show; losing them to hide one message is the wrong trade.
- **`error` for all of wgpu-hal.** Simpler, but hides the warnings that report real problems.

## Consequences

- Default runs show the engine's own lines and wgpu-hal's warnings.
- Warnings of the Vulkan validation layers, when they are turned on, need
  `SLIME_LOG=info,wgpu_hal=warn` to appear.
- The default filter is one constant to adjust as other dependencies join, each change with the
  message that justifies it.
