# 0196. Checks by hand on macOS

- Status: Accepted
- Date: 2026-11-06

## Context

[ADR-0173](0173-the-platforms-of-applications.md) has CI build and test uwui and its desktop host
on GitHub's machines with macOS, and makes macOS supported once a Mac can check it by hand. CI
does not see AppKit's windows, the menu bar, Retina's scales, VoiceOver, text typed through the
system, its dialogs, or Metal on a display.

The owner can now check uwui by hand on macOS 26, on an Intel machine with an AMD GPU. Most users'
Macs have Apple's own chips, whose GPUs draw in tiles, unlike AMD's; macOS 26 is the last release
for Intel's.

## Decision

The owner's decision of 2026-11-06.

- From M6d.7, a milestone that changes uwui's behaviour on macOS adds its checks by hand on macOS
  to the list waiting for the owner, as the Windows virtual machine's are: windows, the menu bar,
  scales, VoiceOver, text input, dialogs, Metal.
- They are written as made on macOS 26, Intel with an AMD GPU.
- They do not count as the Mac that ADR-0173 waits for: support still waits for a check on Apple's
  chips. CI stays as ADR-0173 decides.

## Alternatives considered

- **macOS supported now, on these checks:** sooner, but on a machine that most users do not have,
  and ending with macOS 26.
- **No checks by hand on macOS:** nothing to make, but what CI cannot see left unseen.

## Consequences

- Bugs of AppKit, VoiceOver and text input show before a Mac with Apple's chips checks uwui.
- What only Apple's GPUs do stays unseen until then.
