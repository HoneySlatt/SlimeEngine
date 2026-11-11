# 0213. uwui's reach, its lead, its themes and agents

- Status: Accepted; amends [0173](0173-the-platforms-of-applications.md) (macOS aimed at, not only
  within reach), [0181](0181-uwuis-scope.md) (macOS aimed at; a trial on iOS),
  [0190](0190-uwuis-requirements.md) (macOS's packages; porting a budget; uwui's core ships one
  look, the others official themes of their own; agents measured, and users' agents),
  [0196](0196-checks-by-hand-on-macos.md) (macOS supported at uwui's 1.0 on its checks by hand),
  [0198](0198-screens-of-reference-and-the-rivals-measured.md) (a fifth screen and a gallery; the
  bar's lead) and [0199](0199-uwuis-targets-tools-and-references-narrowed.md) (`uwui bundle`
  packages macOS); amended by [0214](0214-the-engine-under-a-licence-of-its-own-with-a-royalty.md) (uwui paid for by its patrons), [0215](0215-the-engine-measured-against-the-market.md) (uwui measured as the engine is, no look imposed), [0218](0218-one-core-for-agents-in-uwui.md) (actions declared once serve the engine too), [0222](0222-what-a-theme-is.md) (the official themes are crates of data, which may add components of their own) and [0226](0226-an-application-started-in-one-call-each-root-given-its-look.md) (the core's look gains a variant, Plain), then by [0233](0233-uwuis-platforms-apples-chips-ios-and-the-tvs.md) (macOS's CI on Apple's chips; the iOS trial on a Mac; AccessKit's adapter for iOS exists) and [0236](0236-one-trial-of-content-from-outside-for-aerith-and-magnolia.md) (the order of the trials)
- Date: 2026-11-07

## Context

The owner asked three things of uwui on 2026-11-07. An application on it ports more easily than
with Electron or GPUI: Linux, Windows, macOS and the web, then iOS and Android as far as phones
allow. uwui offers the widest and most complex looks, with by far the best performance and
resources close to nothing. And it is the best framework to use with AI agents.

What the decisions held:

- **The intent, unmeasured.** [ADR-0173](0173-the-platforms-of-applications.md) asks for Electron's
  portability without its cost; [ADR-0190](0190-uwuis-requirements.md) has one interface's code
  serve the desktop, the phone and the web. uwui is embedded in a host on each platform
  ([ADR-0181](0181-uwuis-scope.md)), which the web and Android require. Nothing measures how easily
  an application ports.
- **macOS within reach.** CI builds and tests uwui on macOS; the owner checks it by hand in a virtual
  machine, macOS 26 on Intel with an AMD GPU ([ADR-0196](0196-checks-by-hand-on-macos.md)). It is
  supported once a Mac with Apple's chips checks it. `uwui bundle` makes no package for macOS, and
  the system's integrations are named for Linux and Windows alone.
- **iOS, its core built.** CI builds uwui's core for iOS; there is no host, trial nor package.
  AccessKit has no official adapter for iOS on crates.io. The host's API is declared stable at the
  end of uwui's first stage, after trials on Android and the web alone. The owner has no iPhone.
- **A lead of any size.** A program on uwui beats a screen's bar when it beats the best rival on
  each line ([ADR-0198](0198-screens-of-reference-and-the-rivals-measured.md)): by 1 % passes.
  Part of each figure is not a framework's: a window that only presents on the GPU already costs its
  driver's memory, a start and power, and a GPU at 3840 × 2160 and 240 Hz never draws less than a
  floor. On some lines the bar is that floor: 0 frames at rest, one frame from input to present.
- **Effects, unmeasured against rivals.** The four screens of reference hold no effect; each
  effect's power is measured, its figure decided with the effects
  ([ADR-0171](0171-effects-of-the-interface.md)).
- **Six looks shipped.** ADR-0190 has uwui ship its own design system, light, dark and of high
  contrast, and Pro, Material 3 Expressive and Glass, three densities over each. Each look shipped
  covers every component in every density, with its images of reference, under uwui's promise of
  semantic versioning. Pro serves the engine's editor; Material 3 Expressive, Opium; the owner's
  other applications will reuse them.
- **Agents.** uwui's channel and MCP server let an agent read and drive an application in
  development (M6d.9); layouts and Rust's code reload at once
  ([ADR-0212](0212-a-functions-body-reloaded-through-subsecond-and-dx.md)). The engine's MCP server
  adds rights for each agent, a preview before applying and tools found by families
  ([ADR-0209](0209-platforms-the-editor-and-agents.md)); uwui's has none of them. Models know React,
  Flutter and Compose from their training, not uwui. Nothing lets a user's own assistant use a
  finished application. The systems move that way: Android's AppFunctions reached its first beta on
  2026-10-21; Apple's App Intents are the one way for Siri to call an application; Windows lists
  agents' connectors, through MCP, in a registry on the machine. Linux has no such mechanism.

## Decision

The owner's decisions of 2026-11-07.

**macOS aimed at, for uwui's 1.0.** As Linux and Windows: the desktop host and the system's
integrations on macOS, the menu bar, Cmd as the primary modifier, notifications, Now Playing and the
Dock; `uwui bundle` makes an application bundle and a disk image, signed and notarized by Apple,
which needs an Apple developer account. The owner checks it by hand in the virtual machine, and on a
Mac with Apple's chips from time to time, when one is at hand. The documentation names the last
check on Apple's chips and its date; until one is made, it says that Apple's chips are not checked.

**A trial on iOS.** A third bounded trial of two milestones, thrown away, after Android's and before
the API is declared stable: uwui in a UIKit view; the surface lost and made again; rotation; the
system's keyboard with composition; iOS's scrolling; suspend and resume; a first try of VoiceOver,
through an adapter written for it. It runs on Xcode's iOS simulator in the owner's virtual machine.
Frame times, power, touch and VoiceOver on an iPhone are debt until one is at hand. iOS stays out of
1.0's targets: its host, packages and screen reader come when a project asks.

**Porting, a budget.** An application that uses only uwui's APIs runs on Linux, Windows, macOS and
the web with no line written for one platform; on Android and iOS, only its package's configuration
and permissions differ. The budget is provisional until the trials of Android, the web and iOS
confirm or correct it. uwui itself covers what differs between platforms and an application needs:
files and settings, the network, long work. CI builds uwui's examples for every target on each push.
The small application that ADR-0190 writes with each rival before 1.0 is built and run on Linux,
Windows, macOS, the web, Android and iOS's simulator; for each platform, it counts the lines written
for it, the steps from `uwui new` to a package installed, and the package's size; against Electron,
Tauri, Flutter, GPUI and Compose Multiplatform, with a table of which reaches which platform.

**Effects measured.** A fifth screen of reference, rich in effects: glass over moving content, a
background blurred, shapes that morph, a transition on springs from one screen to the next. The owner
chooses it before any work on it; the proposal is Opium's screen of the song playing, in Opium's
Glass theme. It is measured against the rivals that can draw it: Flutter, Compose, Electron, SwiftUI
with Apple's glass on macOS. Beside it, a table of what each rival can draw: refraction, shapes that
morph, shared elements, springs interrupted, HDR, an application's shaders. And a gallery of the
market's mock-ups, chosen with the owner, that uwui reproduces faithfully, each judged image against
mock-up, with its frame's cost. The screen comes in uwui's first stage, beside its first material of
glass; the gallery, before 1.0.

**A lead of three times, on a program's own cost.** On each machine and each screen, a *blank
window*, a program that only clears its window and presents it, is measured with the others. A
program's *own cost* is what it adds above the blank window. uwui's own cost is at most a third of
the best rival's on memory, the start, the processor, power, a frame's time and the package's size.
Where the blank window's figure is the bar, at rest and from input to present, uwui equals it.
Beating each line stays the least a program must do. A figure missed is debt until it is met. Each
effect's power takes its figure from this rule.

**One look in uwui's core; official themes of their own.** uwui's core ships its own design system,
light, dark and of high contrast, and the three densities: what every component needs to be usable
and accessible at once. Glass, Material 3 Expressive and Pro become official themes of uwui: in its
repository, each a crate of its own with its own version, which an application adds if it wants it;
the core depends on none. Their images of reference run in uwui's CI, so that a change of uwui that
breaks one is refused before it enters. The glass material stays an effect of uwui's core
(ADR-0171). Apple's name, Liquid Glass, is not used: the theme is Glass. A catalogue of the
community's themes comes later, in a repository of its own.

**Agents measured, and protected as in the engine.** uwui's MCP server takes the engine's
protections of ADR-0209: rights for each agent, a preview before applying, tools found by families.
An *agent bench* gives a fixed set of tasks, a screen from a mock-up, a feature added, a bug fixed, a
port to the web, to the same agents with the same words, with uwui and with GPUI and gpui-kit,
Flutter, Compose, Electron, Tauri and Slint; it measures success, its tests passed and its image
matching, the tries, the tokens and the time, each figure with its models, their versions and its
date. It runs at each release of uwui. uwui ships documentation written for agents, a compact
reference and a skill, and `uwui new` writes an `AGENTS.md`. The provisional goal: agents succeed
with uwui at least as often as with the best rival, on fewer tokens.

**A user's agents use an application.** An application declares its actions once: a name, its
parameters, a description and the permission that it asks. Before 1.0, a local MCP server lets the
user's assistant call them on the desktop. The user consents agent by agent; each action goes through
the interface's own commands, so that it is undone and logged; no secret is given; uwui still sends
nothing over the network of its own accord. The declaration is checked against AppFunctions, App
Intents and Windows' registry when it is designed, so that an adapter never changes it. Each adapter
comes when a pilot needs it and its system offers it; Android's comes in the stage where Opium ships
on Android.

## Alternatives considered

- **macOS kept within reach until a Mac with Apple's chips**: no promise unchecked, but porting to
  macOS stays a build without a package, where the rivals ship macOS first.
- **macOS packaged without a signature**: no account to pay, but macOS refuses unsigned
  applications by default.
- **iOS left as it is**: nothing to do, but the host's API declared stable without having met iOS.
- **iOS aimed at for 1.0**: the whole promise, but no project behind it, which ADR-0181 requires of a
  target, and a screen reader's adapter to write whole.
- **Porting measured without a budget**: figures, but no rule that an API forcing a test of the
  platform breaks.
- **Effects measured by their power alone**, or **a fifth screen without a gallery**: cheaper, but
  the claim of the widest looks unmeasured, or measured on one screen.
- **Beating each line, as before**: always within reach, but a lead of 1 % passes.
- **Twice the best rival on the whole figure**: simple, but out of reach where the hardware or the
  driver sets the floor, a debt that no work could settle.
- **Twice the best rival's own cost**: proposed; the owner chose three times.
- **Six looks in uwui's core**: everything at once, but years of upkeep of tastes under the core's
  versioning, a theme's change a break of uwui.
- **No look at all**: the most neutral framework, but components unusable without work, accessibility
  not given, and writing an application no longer easier than with gpui-kit.
- **The official themes in a repository of their own now**: a clean split, but before 1.0 each
  change of a component would cross two repositories, the themes broken in between, and their images
  of reference would no longer guard uwui's drawing. A crate of its own already confines a theme to
  uwui's public API.
- **The themes kept in Opium and copied**: nothing to build, but copies that drift.
- **No agent bench**, or **one without rivals**: no cost, or less, but no figure on where uwui starts
  behind, or no comparison.
- **A user's agents left out**: no new surface to keep safe, but applications closed to assistants
  as the systems open to them.
- **Every system's mechanism for 1.0**: Gemini, Siri and Windows' registry at once, but three bridges
  to Kotlin, Swift and the package's manifest, on mechanisms in beta or preview that may change, and
  App Intents serving mostly iOS, which 1.0 does not aim at.

## Consequences

- uwui's roadmap takes these decisions: its targets, the fifth screen and the gallery, the trial on
  iOS, the budgets of porting and of the lead, the blank window among the programs measured, its
  themes, the agent bench and a user's agents.
- An Apple developer account, paid each year, before macOS's first package.
- The four screens' bars gain their own costs when the blank window is first measured beside them.
- The trial on iOS opens its debt: the measures on an iPhone.
- uwui's API grows what the web and the phones lack and an application needs, files and settings
  among them, so that no application writes a line for one platform.
- Three crates of themes in uwui's repository, tested in its CI, versioned apart from the core.
- The agent bench costs tokens at each release; its figures change with the models and name them.
- A user's agents add a surface to keep safe: consent, rights, undo and logs, tested as the rest.
