# 0233. uwui's platforms: Apple's chips, iOS behind a project, and the televisions

- Status: Accepted; amends [0181](0181-uwuis-scope.md) (Android TV aimed at, tvOS within reach
  through a trial after uwui's first stage; iOS aimed at once a project of the owner's is accepted
  on Apple's TestFlight), [0196](0196-checks-by-hand-on-macos.md) (checks by hand on macOS are
  made on a Mac with Apple's chips, not in a virtual machine) and
  [0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md) (macOS's CI on Apple's chips; the iOS
  trial on a Mac; AccessKit's adapter for iOS exists)
- Date: 2026-11-11

## Context

The owner is designing Aerith, an open-source media client on uwui, for Linux, Windows, macOS,
iPhone, Apple TV, Android phones and Android TV. Aerith is a later client of uwui, after its first
stage. ADR-0181 aims uwui only at platforms with a project behind them; it put Android TV and
Apple TV out of scope, and ADR-0213 kept iOS to a trial on a simulator for want of one.

What changed, read on 2026-11-11:

- Xcode 27 "will only install and run on Apple silicon Macs", and macOS 26 is the last release for
  Intel ([Xcode 27's notes](https://developer.apple.com/documentation/xcode-release-notes/xcode-27-release-notes)).
  Apple raises the SDK that uploads need each spring.
- macOS's licence allows virtual copies only "on each Apple-branded computer you own or control"
  ([macOS Tahoe's licence](https://www.apple.com/legal/sla/docs/macOSTahoe.pdf), §2B(iii)). A
  project that writes "checked on macOS" cannot rest on a virtual machine on other hardware.
- GitHub's macOS runners on Apple's chips are free for public repositories; they hold the iOS and
  tvOS SDKs and simulators. `aarch64-apple-ios` and `aarch64-apple-tvos` are Tier 2 targets of
  Rust.
- `accesskit_ios` exists, 0.2.2 on 2026-10-25 ([crates.io](https://crates.io/crates/accesskit_ios));
  ADR-0213's context said the opposite. AccessKit has no adapter for tvOS.
- winit's UIKit backend implements `UIKeyInput` alone: no composition through `UITextInput`, no
  focus for tvOS.
- On tvOS, "only the focus engine can explicitly update focus"; an application cannot move it in a
  direction ([About focus interactions for Apple TV](https://developer.apple.com/documentation/uikit/about-focus-interactions-for-apple-tv)).
  tvOS has no channel but the App Store; whether App Review accepts an interface drawn wholly by
  the application is unknown.
- Android TV's quality checklist, even its lowest tier, asks a memory limit on low-memory devices,
  32- and 64-bit builds with 16 KB pages from 2026-08-01, the D-pad's centre and the play key
  pausing playback, no ambient mode during playback, nothing cut by overscan, and TalkBack
  ([TV app quality](https://developer.android.com/docs/quality-guidelines/tv-app-quality)).
- Flutter lists neither tvOS nor Android TV; Compose Multiplatform has only tvOS's remote buttons;
  Google gives Android TV components of its own.

The owner has an Apple TV 4K of 2022 (A15: HDR10+, Dolby Vision, matching the content's range and
rate), a Pixel, and no Mac with Apple's chips, iPhone nor Android TV box. A challenger's review
changed the first recommendation: it had a tvOS trial before uwui's API is declared stable, and
Android TV as cheap.

## Decision

The owner's decision of 2026-11-11.

- **macOS, aimed at for 1.0, on Apple's chips.** uwui's CI builds and tests on GitHub's runners
  with Apple's chips from the move (M6d.11). Checks by hand are made on a Mac with Apple's chips; a
  virtual machine on another computer is no longer one of uwui's checks. The owner buys no Mac for
  now: until one is at hand, macOS is checked by CI alone, and its checks by hand are debt, written
  as such.
- **iOS, aimed at for 1.0 behind Aerith.** It becomes a target once a minimal build of Aerith is
  accepted by Apple's review for TestFlight; Aerith's addons may fall under the App Review
  Guidelines' 2.5.2. Until then iOS stays as ADR-0213 has it. The iOS trial of uwui's first stage
  keeps its place and its content, through uwui's own UIKit host rather than winit's, with
  `accesskit_ios`; it needs a Mac with Apple's chips, bought before it starts. iOS 17 is the
  minimum, which Xcode 27 debugs. An iPhone measures frames, power, touch and VoiceOver before 1.0.
  iPadOS's stylus, pointer and windows stay out of 1.0, as debt.
- **Android TV, aimed at for 1.0.** In the same Kotlin host as the phone: the D-pad, Back and the
  media keys, which pass the focus by; the screen kept on while a video plays; TalkBack driven by
  the D-pad; the couch density; 32-bit builds; a non-transparent window. `uwui bundle` writes the
  leanback manifest and the banner. A third milestone of uwui's Android trial runs it on a box,
  before the API is declared stable. The machines gain a TV tier: a Xiaomi TV Box S of the third
  generation (Amlogic, 2 GB, a Mali-G310, Google TV), the lowest box supported. The porting budget
  counts a couch screen's lines apart, as a form factor's, not a platform's.
- **tvOS, within reach through a trial after the first stage.** CI builds uwui for tvOS from the
  move. The iOS trial's report says on paper how uwui's focus maps onto tvOS's focus engine: uwui's
  focusable elements mirrored as the system's focus items, the system moving the focus, uwui
  drawing it. A minimal build of Aerith goes to TestFlight for tvOS. A trial of two milestones on
  the owner's Apple TV follows uwui's first stage, if Aerith passes; its report decides whether
  tvOS is a target of 1.0 or after.
- **uwui's matrix of platforms**, which SlimeEngine's M6d.10 writes
  ([ADR-0199](0199-uwuis-targets-tools-and-references-narrowed.md)), names the system versions that
  CI tests, as Flutter's does.

## Alternatives considered

- **The targets as they were**: nothing to buy, but Aerith without uwui on televisions and Apple's
  devices, and macOS's checks resting on a virtual machine that the licence does not allow.
- **Every platform a target of 1.0, with a tvOS trial before the API is declared stable**: the whole
  promise, but about two more milestones in the first stage to learn what Apple's documents and CI
  already say, before knowing whether Apple accepts Aerith.
- **Android TV in Aerith's trial**: one trial fewer to cut, but the host's and the focus's needs met
  after the API is shaped.
- **tvOS's focus driven by uwui from the remote's presses**: simpler, but against the system's
  inertia, focus effects and accessibility, which only its focus engine gives.
- **A rented Mac**: no purchase, but no screen nor sound at hand for checks by hand.

## Consequences

- The documents that named the virtual machine as macOS's check change with this one: uwui's
  roadmap and ADR-0213's trial on iOS.
- Before the iOS trial: a Mac with Apple's chips (the cheapest that runs Xcode 27 well, on
  2026-11-11, a refurbished Mac mini M4 with 16 GB at 809 € on Apple's French store). Before 1.0, if
  iOS is a target: an iPhone, with ProMotion to check 120 Hz (an iPhone 17, 1,119 €). An Android
  TV box (84 €). Apple's developer account, about 99 € a year, which macOS's packages already need,
  and TestFlight too.
- wgpu's fixes for Metal's colour spaces and composited alpha are still unreleased on 2026-11-11;
  uwui's work on Apple waits for the release that has them.
- Each target adds checks by hand at each milestone; Apple's examples build each night rather than
  at each push, so that 3-core runners are not the limit.
- Aerith's TestFlight reviews, for iOS and tvOS, are Aerith's to run; their results come back here.
