# 0249. The host's API: its items in three channels, and how each maps on five platforms

- Status: Accepted; amends [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (an
  answer's three states; questions change uwui's caches); amended by
  [0251](0251-the-engines-host-clock-and-its-times.md) (the wake built with uwui's executor) and
  [0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md) (the moves between two frames, the wheel's phase, a field's
  purpose, the pointer's entering and whether the cursor shows wait for their readers)
- Date: 2026-11-13

## Context

[ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) cut M6d.5 in five and gave
the host's API its shape: *events* that the host gives and that answer at once, *questions* that
the host asks and that uwui answers from its live state, *outputs* that the host reads after each
event and each frame, and one callback, the *waker*. M6d.5 builds what has a reader; the rest is
described here, so that the API can carry it. This ADR amends ADR-0248 where the prototype showed it short: an event's answer has three
states, and questions change uwui's caches. It lists every item of
[M6d.5's card](../stage-6d.md#m6d5--the-hosts-api), the channel that carries it, the part that
builds it, and how it maps on the platforms that uwui aims at ([ADR-0181](0181-uwuis-scope.md),
[ADR-0213](0213-uwuis-reach-its-lead-its-themes-and-agents.md)).

Words used below:

- A *host* is the program that owns the event loop and the platform's objects, and drives uwui:
  the engine's `slime-app`, uwui's desktop host of M6d.7, a Kotlin activity, a web page.
- A *target* is where a root draws ([ADR-0226](0226-an-application-started-in-one-call-each-root-given-its-look.md)):
  a window, a region of the host's image, a texture. Every event and output names its target.
- A *step* is what the host runs when it wakes: the events that came, the questions they raised,
  then a frame or a wake ([ADR-0227](0227-the-time-that-motion-reads.md)).

A prototype without a window, thrown away, tried the three hard cases on uwui's `Ui` on
2026-11-12 (branch `m6d5a-prototype`): an input method's question asked right after its commit,
with no frame between, in UTF-8 and UTF-16; copy, cut and paste as events, a paste of uwui's own
key asked for and answered by its number, dropped when the focus moved; two targets with a button
at the same place. Each passed. Its costs over a field of 4,000 bytes are in
[`measurements.md`](../measurements.md#m6d5a-the-hosts-api-tried-on-a-prototype), on the high
tier alone: an edit with its question takes about a quarter of the editor's frame at 240 Hz, and a
host that asks one rectangle for each character of a composition pays a little over 0.1 ms for
each. It found what this ADR adds to ADR-0248: an answer's third state, the text's
unit and bound named by the host, a paste that no field asked for, and where uwui assumes one
surface.

The facts were read on 2026-11-12 in the platforms' current documentation and in winit's sources,
listed under [Sources](#sources). winit 0.30.13 is the engine's; 0.31's betas
([ADR-0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md)) are noted where they differ.

## Decision

### The items, by channel

**Events**, each with its target, its time and the time's source (ADR-0227). Each answers at once:
not taken; taken; or taken and left to the platform, as the web's keys of copy, cut and paste,
kept from the game but not prevented, so that the browser fires its own events. The answer also
carries what must happen inside the platform's handler:

| Item | What it carries | Built by |
|---|---|---|
| Target | added, resized, its scale changed, lost, made again, removed | M6d.5e |
| Look | each root's look given as a value, replaced while it runs | its reader |
| Insets | the parts of a target that the system covers, and their changes | its reader |
| Pointer | entered, left, moved (gathered for each frame, with the moves between), a button; its kind: mouse, pen, finger | M6d.5c (mouse), its reader (pen, finger) |
| Wheel | lines or pixels, its phase | M6d.5c |
| Keyboard | the physical key, the logical key, its text, the modifiers, a repeat | M6d.5c |
| Input method | composition with its range, commit; deletion around the caret | M6d.5c; its reader (a host that gives it) |
| Edit commands | copy and cut, answered with the selection; paste, carrying its data | M6d.5c |
| Clipboard's answer | the data that a read request numbered asked for | M6d.5c |
| Clipboard's rights | whether the program may read the clipboard without the person's gesture | its reader |
| Focus | the target given or taken the system's focus, and the element that takes it then | M6d.5e |
| Lifecycle | suspended, resumed with the time spent suspended; shown, hidden, covered; low memory | M6d.5e; the time suspended M6d.5b |
| Preferences | the text's size and less motion, and their changes; light or dark, contrast, forced colours with their roles' colours, less transparency, saving power | M6d.5e (the two); the reader of uwui's first stage |
| Frame | the frame's time and source; its deadline where given, the display's rate and whether it varies | M6d.5b; their reader, the caps of uwui's first stage |
| Root's clock | each root's time and speed, given at each frame | M6d.5b |
| Wake | the time of a wake that runs uwui's timers without drawing | M6d.5b |
| Screen reader's action | an action on a node, with the number of the update it was asked on | M6d.5d |
| Touch, gamepads | fingers with their identities; a gamepad's buttons and sticks | their reader |
| Application's events | files and addresses to open, opened again, a second start, the language changed | their reader |
| Civil time | the time of day, its zone, its offset from UTC, its next change | its reader |

**Questions**, answered from uwui's live state, the edits of the same step included. They change
nothing that the person sees; in Rust they take `&mut Ui`, since a question may lay a field's text
out or settle its scroll on demand:

| Item | What it answers | Built by |
|---|---|---|
| Text around the caret | up to a bound before and after, the selection, the composition, in the unit the host names | M6d.5c |
| A range's rectangle | the box of a range of characters, in the target's physical pixels | M6d.5c |
| The character at a point | its index | M6d.5c |
| Whose point | whether a point of the target is uwui's, for decorations and drops | M6d.5c |
| An element's place | the box of an element, for the agents' capture from the host's image | the interface's family, moved into uwui at M6d.9a1b |
| The whole tree | AccessKit's first whole tree, on a screen reader's demand | M6d.5d |

**Outputs**, for each target, only what changed since the host last read:

| Item | What it asks | Built by |
|---|---|---|
| Frame and wake | the next frame due, the next wake | M6d.5b |
| Cursor | its shape, whether it shows | M6d.5c |
| Input method's state | on or off; the field's purpose and hints; the caret's area; the text around it | M6d.5c |
| Clipboard | data to write; a read requested, numbered | M6d.5c |
| Accessibility | the tree's update, numbered | M6d.5d |
| Rate | the highest rate that the motions ask | its reader |
| Menus | a model of menus, items and their shortcuts, that a menu bar of the system takes | its reader |
| A layer's own target | a menu, a tooltip or a dialog that asks its host for a window of its own ([ADR-0190](0190-uwuis-requirements.md)) | its reader |

**The waker**: the host gives uwui a `std::task::Waker` that any thread may call; it only asks the
host for a step. M6d.5b builds it. The agents' sessions use the same one
([ADR-0246](0246-one-core-for-agents-cut-in-five-a-session-without-input-or-output.md)); until
then, the engine's channel wakes the engine's loop through `slime_platform::Waker`, as now.

### Rules

- uwui's core runs on the platform's thread of the interface. A question asked while an event is
  handled sees that event's edits, laid out; a field whose box grows with its text answers from
  its last frame's box until the next frame.
- Positions, the pointer's, a range's rectangle, the character at a point and the input method's
  area, are in the target's physical pixels, as uwui takes them today; a root's size is in logical
  pixels, with the scale that the host gives. Hosts whose platform counts in logical pixels, the
  web, Apple's and Wayland, multiply by the scale at the boundary. Offsets into text are in UTF-8
  bytes in uwui; the host names the unit it needs, UTF-16 code units on Android, Apple, the web
  and Windows, code points for Android's deletions, and uwui converts.
- The host names the bound of the text around the caret and whether a composition in progress
  is part of it; the answer gives the text's offset into the whole and the composition's range,
  so that a host cuts what its platform does not want.
- Each reader of meaning, AccessKit's translation and an agent's session, follows its own changes
  rather than taking them from the others.
- A read of the clipboard carries a number; an answer whose number is not awaited, or whose field
  lost the focus, is dropped. Each accessibility update carries a number, so that a screen
  reader's action asked on a tree that changed since is recognised.
- A paste typed on the web or on iOS goes to the platform, which gives it back as a paste event; a
  paste of uwui's own shortcuts elsewhere is a read requested, then its answer. A read that the
  program asked for, with no field, as the editor's paste, brings its answer to the program as an
  action.
- Copy and cut answer with the selection inside the event's handler, since Safari writes to the
  clipboard only during the gesture.

### The table

For each item, how it maps or why it needs nothing. *winit* is 0.30.13; where it gives nothing, the
cell names what the host reads on Wayland, X11, Windows and macOS. *Engine* is what `slime-app`
and `slime-platform` give today, and what M6d.5 adds. Each API named is documented in the
[Sources](#sources), by platform.

**Target and frames**

| Item | winit | Android | Web | UIKit | AppKit | Engine |
|---|---|---|---|---|---|---|
| Made, resized | `Resized`, `inner_size` (0.31: `can_create_surfaces`, `SurfaceResized`) | `SurfaceHolder.Callback.surfaceCreated`, `surfaceChanged` | `ResizeObserver`, `devicePixelContentBoxSize` (not Safari) | `layoutSubviews`, `CAMetalLayer` | `NSView` with `CAMetalLayer`, `windowDidResize` | `Event::Resized`, the window's size read at each frame; M6d.5e gives it |
| Scale | `ScaleFactorChanged`, `scale_factor` (Wayland: fractional-scale) | `Configuration.densityDpi` | `devicePixelRatio`, `matchMedia("(resolution: …)")` | `contentScaleFactor` | `backingScaleFactor`, `windowDidChangeBackingProperties` | `Event::ScaleFactorChanged`, unread; M6d.5e gives it |
| Lost, made again | Android only (`suspended`; 0.31: `destroy_surfaces`) | `surfaceDestroyed`, then `surfaceCreated` | `webglcontextlost`, `GPUDevice.lost` | no loss documented | `MTLDevice`'s removal notifications, for a removable GPU | a lost surface ends the run (`ExitReason::SurfaceLost`); M6d.5e makes it again |
| Frame's time | none: Wayland `wp_presentation_feedback.presented`; X11 Present's `PresentCompleteNotify` (`ust`, `msc`); Windows `IDXGISwapChain::GetFrameStatistics` (flip model or full screen); macOS `CADisplayLink.timestamp` (macOS 14), `MTLDrawable.presentedTime` | `Choreographer.postVsyncCallback`, `FrameTimeline.getExpectedPresentationTimeNanos` (API 33) | `requestAnimationFrame`'s time | `CADisplayLink.targetTimestamp` | `NSScreen.displayLink`, `CADisplayLink.targetTimestamp` (macOS 14) | none: Wayland's `wp_presentation`, Windows' DXGI statistics at M6d.7; the frame's start, M6d.5b |
| Deadline | none: Wayland `wp_commit_timer_v1.set_timestamp` (staging); X11 Present's `target_msc`, or a time with `PresentOptionUST`; Windows `IPresentationManager::SetTargetTime` (build 22000); macOS `MTLDrawable.present(at:)` | `FrameTimeline.getDeadlineNanos` (API 33) | none | none beside the frame's time | none beside the frame's time; `MTLDrawable.present(at:)` asks a time | none |
| Display's rate, varies | `MonitorHandle::refresh_rate_millihertz` (Android, the web: none); Wayland `wp_presentation` `refresh`; X11 RandR's mode; Windows `DwmGetCompositionTimingInfo`; macOS `NSScreen` | `Display.getRefreshRate` | measured from `requestAnimationFrame` | `UIScreen.maximumFramesPerSecond` | `NSScreen.maximumFramesPerSecond`, `minimumRefreshInterval` | none: `AppConfig::max_frame_rate`, 60 by default |
| Rate asked | none: Wayland, X11 none found; Windows `IPresentationManager::SetPreferredPresentDuration` (build 22000); macOS `CADisplayLink.preferredFrameRateRange` (macOS 14) | `Surface.setFrameRate` (API 30; its strategy, API 31) | none found | `CADisplayLink.preferredFrameRateRange` | `CADisplayLink.preferredFrameRateRange` | none |
| Frame asked | `request_redraw` | `Choreographer.postFrameCallback` | `requestAnimationFrame` | `CADisplayLink.isPaused` | `CADisplayLink.isPaused` | `Context::request_frame`, `Ui::needs_frame` |
| Waker | `EventLoopProxy` (0.31: `wake_up`, coalesced) | `Handler.post`, NDK `ALooper_wake` | `MessagePort.postMessage` on a `MessageChannel`'s port | a `CFRunLoopSource` signalled, winit's proxy | a `CFRunLoopSource` signalled, winit's proxy | `slime_platform::Waker`, `slime-app`'s `wake.rs` |
| Suspend's time | none: Linux `CLOCK_BOOTTIME`; Windows the performance counter; macOS `mach_continuous_time` | `SystemClock.elapsedRealtimeNanos` | `performance.now()`, as the browser counts | `mach_continuous_time`, documented for macOS | `mach_continuous_time` | `std::time::Instant`, which stops in a suspend on Linux; M6d.5b |
| Target removed | `WindowEvent::Destroyed`, the `Window` dropped | `surfaceDestroyed`, `Activity.onDestroy` | none: the canvas removed from the page, seen by a `MutationObserver` | `sceneDidDisconnect` | `windowWillClose` | the window closed ends the run |
| Wake at a time | `ControlFlow::WaitUntil` | `Handler.postAtTime`, `postDelayed` | `setTimeout` | `Timer`, `CFRunLoopTimer` | `Timer`, `CFRunLoopTimer` | `LoopControl::WaitUntil` for `Ui::next_change`; M6d.5b |
| A layer's own target | none (0.31: `WindowType::Popup`; Wayland `xdg_popup`) | `PopupWindow` | none found: inside the canvas | none found: inside the scene | `NSWindow.addChildWindow`, `NSPanel` | none: layers drawn in the window |
| Root's clock, a look replaced | needs nothing: uwui's own, the host's game time | needs nothing | needs nothing | needs nothing | needs nothing | the game's time, M6d.5b; a look, its reader |

**Input**

| Item | winit | Android | Web | UIKit | AppKit | Engine |
|---|---|---|---|---|---|---|
| Pointer | `CursorMoved`, `CursorEntered`, `CursorLeft`, `MouseInput` (0.31: `PointerMoved` with `PointerSource`) | `MotionEvent`, `getToolType`, `getHistoricalX` | `PointerEvent`, `getCoalescedEvents` | `UITouch`, `UIPointerInteraction` | `NSEvent` mouse events | `Event::CursorMoved`, `MouseButton`; M6d.5c |
| Pen | its pressure, and the pencil's altitude, as `Touch.force` (0.31: `TabletTool` on Wayland, Windows, the web; the pencil on iOS) | `TOOL_TYPE_STYLUS`, pressure, tilt | `PointerEvent.pressure`, `tiltX`, `altitudeAngle` | `UITouch.force`, `altitudeAngle` | `NSEvent.pressure`, `tilt` | none |
| Wheel | `MouseWheel` lines or pixels, `TouchPhase` | `AXIS_VSCROLL`, `AXIS_HSCROLL` | `WheelEvent.deltaMode` | pan recognisers | `scrollingDeltaY`, `hasPreciseScrollingDeltas`, `phase` | `Event::MouseWheel`; M6d.5c |
| Touch | `Touch` with `id` (macOS: none) | `getPointerId`, `ACTION_POINTER_DOWN` | `PointerEvent` `pointerType: "touch"` | `touchesBegan`, the `UITouch` kept through its sequence | none on the screen; `NSTouch` for the trackpad's gestures | none, debt (issue #3) |
| Event's time | none (issue #1194, pull request #4562 open): Wayland events' `time` in milliseconds, `zwp_input_timestamps_v1` in nanoseconds; X11 events' `time` in milliseconds; Windows `GetMessageTime`, `POINTER_INFO.PerformanceCount`; macOS `NSEvent.timestamp` | `MotionEvent.getEventTimeNanos` (API 34), `getEventTime` | `Event.timeStamp` | `UITouch.timestamp` | `NSEvent.timestamp` | none; stamped at reception, M6d.5b |
| Keyboard | `KeyboardInput` with `physical_key`, `logical_key`, `text`, `repeat` | `KeyEvent.getKeyCode`, `getScanCode`, `getUnicodeChar`, `getRepeatCount` | `KeyboardEvent.code`, `key`, `repeat`, `isComposing` | `UIPress.key`, a `UIKey` (iOS 13.4), `UIKeyInput.insertText` | `NSEvent.keyCode`, `characters`, `isARepeat` | `Event::Key`, `ModifiersChanged`; `Ui::key` answers |
| Edit commands | none: the keys, through uwui's shortcuts | `InputConnection.performContextMenuAction` (copy, cut, paste); `KEYCODE_COPY`, `KEYCODE_CUT`, `KEYCODE_PASTE` | `copy`, `cut`, `paste` events | `UIResponderStandardEditActions` `copy(_:)`, `cut(_:)`, `paste(_:)` | `copy:`, `cut:`, `paste:` sent to the first responder by the Edit menu | the editor's shortcuts; M6d.5c |
| Whose point | `drag_window`, `set_cursor_hittest` (Android, iOS, the web: none); `DroppedFile`, `HoveredFile` (paths alone); Windows `WM_NCHITTEST` beneath | `View.OnDragListener` | `dragover`, `drop`, `document.elementFromPoint` | `UIView.hitTest(_:with:)`, `UIDropInteraction` | `NSView.hitTest`, `mouseDownCanMoveWindow`, `NSDraggingDestination` | `Ui::pointer_button`'s answer; dropped files: none, debt (issue #3) |
| Gamepads | none: `gilrs` on Wayland, X11, Windows, macOS | `InputDevice.SOURCE_GAMEPAD`, `onGenericMotionEvent` | `navigator.getGamepads` | `GCController` | `GCController` | none, debt (issue #3) |
| Screen reader's action | `accesskit_winit` `ActionRequested` | `accesskit_android` | none: no AccessKit adapter | `accesskit_ios` | `accesskit_winit` (`accesskit_macos`) | `Event::Accessibility`, `slime-app`'s `accessibility.rs`; M6d.5d |
| Whole tree, updates | `accesskit_winit` `InitialTreeRequested`, `Adapter::update_if_active` | `accesskit_android` `update_if_active` | none: no AccessKit adapter | `accesskit_ios` `update_if_active` | `accesskit_winit` (`accesskit_macos`) | `Window::update_accessibility`, `ScreenReaders`; M6d.5d |
| An element's place | needs nothing: uwui's own | needs nothing | needs nothing | needs nothing | needs nothing | the channel's capture, cropped from the window's image (M6d.9a1b) |

**Input methods, both ways**

| Item | winit | Android | Web | UIKit | AppKit | Engine |
|---|---|---|---|---|---|---|
| Composition, commit in | `Ime::Preedit`, `Ime::Commit` (Android, iOS: none) | `InputConnection.setComposingText`, `commitText` | `CompositionEvent` on a hidden field; `EditContext` `textupdate` (Chromium only) | `UITextInput.setMarkedText`, `insertText` | `NSTextInputClient.setMarkedText`, `insertText` | `Event::Ime`, `Ui::ime` |
| Deletion around the caret | none (0.31: `Ime::DeleteSurrounding`): Wayland `zwp_text_input_v3.delete_surrounding_text`; X11 XIM's `XNStringConversionCallback`; Windows TSF `ITextStoreACP::SetText`; macOS `insertText` with `replacementRange` | `deleteSurroundingTextInCodePoints` (API 24) | `EditContext` `textupdate` ranges | `UITextInput.replace` | `insertText` with `replacementRange` | none; M6d.5c |
| Text around the caret, out | none (0.31: `ImeSurroundingText`): Wayland `set_surrounding_text`, 4,000 bytes at most; X11 XIM's string conversion; Windows TSF `ITextStoreACP::GetText`, IMM32 `IMR_DOCUMENTFEED`; macOS `attributedSubstring` | `getTextBeforeCursor`, `getSurroundingText` (API 31), asked synchronously | `EditContext.updateText`; else the hidden field's value | `UITextInput.text(in:)`, asked synchronously | `attributedSubstring(forProposedRange:)`, asked synchronously | none; M6d.5c |
| Range's rectangle | `set_ime_cursor_area` (one area) | `CursorAnchorInfo` through `requestCursorUpdates` | `EditContext.updateCharacterBounds` | `UITextInput.firstRect(for:)`, `caretRect` | `firstRect(forCharacterRange:)`, asked synchronously | `Ui::text_input`'s caret area, `Window::set_ime_area` |
| Character at a point | none: Wayland, X11 none found; Windows TSF `ITextStoreACP::GetACPFromPoint`; macOS `characterIndex(for:)` | none found | none found | `closestPosition(to:)` | `characterIndex(for:)` | none; M6d.5c |
| Field's purpose, keyboard shown | `set_ime_purpose` (Wayland), `set_ime_allowed` | `EditorInfo.inputType`, `showSoftInput` | `inputmode`, `enterkeyhint`; `virtualkeyboardpolicy` | `UITextInputTraits.keyboardType`, `becomeFirstResponder` | `NSTextContent.contentType`; no soft keyboard | `Window::set_ime_allowed`; M6d.5c gives the purpose |

**Clipboard, lifecycle, preferences and the rest**

| Item | winit | Android | Web | UIKit | AppKit | Engine |
|---|---|---|---|---|---|---|
| Clipboard | none (0.31: asynchronous `DataTransferReceived` on Wayland, X11, Windows, macOS) | `ClipboardManager`, synchronous, focused application alone (Android 10) | `navigator.clipboard`, asynchronous, permission or a gesture; `paste` and `copy` events | `UIPasteboard`, a prompt on reads | `NSPasteboard` | `slime_platform::Clipboard` behind `Ui::set_clipboard`, synchronous; M6d.5c |
| Clipboard's rights | none needed on X11 and Windows; Wayland gives the selection to the client with the keyboard's focus alone | the focused application alone (Android 10) | a gesture or the `clipboard-read` permission | a prompt on reads without the person's intent | unconfirmed: a prompt on reads by the program, to check when the item is built | needs nothing |
| Focus | `Focused` | `Activity.onWindowFocusChanged` | `focus`, `blur` | `UIWindow.becomeKey`, `resignKey` | `windowDidBecomeKey` | `Event::Focused`, not given to uwui; M6d.5e |
| Suspended, resumed | `resumed` once at the start everywhere, `suspended` on Android, iOS and the web; the system's resume: Linux logind's `PrepareForSleep`; Windows `WM_POWERBROADCAST` `PBT_APMRESUMEAUTOMATIC`; macOS `NSWorkspace.didWakeNotification` | `onStop`, `onStart` | `pagehide`, `pageshow` | `sceneDidEnterBackground`, `sceneWillEnterForeground` | `NSWorkspace.didWakeNotification`; `applicationDidHide`, `applicationDidUnhide` for hidden | none; the system's resume at M6d.7 (ADR-0227); M6d.5e gives what winit gives |
| Covered | `Occluded` (Android, Wayland, Windows: none): X11 `VisibilityNotify`; Wayland `xdg_toplevel`'s `suspended` state; Windows `IDXGIFactory2::RegisterOcclusionStatusWindow`; macOS `windowDidChangeOcclusionState` | `onStop` | `visibilitychange` | `sceneDidEnterBackground` | `windowDidChangeOcclusionState` | a window that holds its frames on Wayland (ADR-0187); M6d.5e |
| Low memory | `memory_warning` (Android, iOS; on Android through `onLowMemory`, which API 34 no longer calls): Linux the kernel's memory pressure, `/proc/pressure/memory`; Windows `CreateMemoryResourceNotification`; macOS a dispatch source of memory pressure | `onTrimMemory` | none found | `applicationDidReceiveMemoryWarning` | a dispatch source of memory pressure | none; M6d.5e passes winit's |
| Forced colours, high contrast | none: Windows `SPI_GETHIGHCONTRAST`, `WM_SETTINGCHANGE`; macOS `accessibilityDisplayShouldIncreaseContrast`, not forced colours; Linux the portal's `contrast` | `UiModeManager.getContrast` (API 34); `AccessibilityManager.isHighContrastTextEnabled` (API 36); no forced colours found | `forced-colors`, `prefers-contrast` | `isDarkerSystemColorsEnabled`, `accessibilityContrast`; no forced colours found | `accessibilityDisplayShouldIncreaseContrast`; no forced colours found | none: the reader at uwui's first stage |
| Light or dark, contrast, less motion, less transparency, text's size | `ThemeChanged` (Windows, macOS, the web); else the portal's `org.freedesktop.appearance` on Linux; on Windows, `UISettings` for motion, transparency and the text's size | `Configuration.uiMode`, `fontScale`; `UiModeManager.getContrast` (API 34); `ANIMATOR_DURATION_SCALE` at 0, read as less motion | `prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency` (not Safari); no text's size | `UITraitCollection`, `isReduceMotionEnabled`, `isReduceTransparencyEnabled`, `preferredContentSizeCategory` | `effectiveAppearance`, `accessibilityDisplayShouldReduceMotion`, `…ReduceTransparency`, `…IncreaseContrast` | `ColorSchemeChanged` on Windows; `Ui::set_preferences` set by the program; the reader at uwui's first stage |
| Insets | none (0.31: `safe_area`, iOS, macOS and the web) | `WindowInsets.getInsets(systemBars, displayCutout, ime)` (API 30) | `env(safe-area-inset-*)`; `env(keyboard-inset-*)` in Chromium | `safeAreaInsets`, `UIKeyboardLayoutGuide` | `NSView.safeAreaInsets` (macOS 11): the title bar; the notch, as winit 0.31 adds it | none: a window's insets are zero on Linux and Windows |
| Application's events | none: macOS `application(_:open:)` through a delegate; Linux `org.freedesktop.Application` | `Intent.ACTION_VIEW`, `onNewIntent`, `ACTION_LOCALE_CHANGED` | `languagechange`; `launchQueue` in Chromium, for installed applications | `scene(_:openURLContexts:)` | `application(_:open:)`, `applicationShouldHandleReopen` | dropped files: none, debt (issue #3) |
| Menus | none: `muda` on Windows, macOS, GTK | `Menu`, `PopupMenu` | `contextmenu` | `UIMenuBuilder`, `UIContextMenuInteraction` | `NSMenu`, `NSApplication.mainMenu`; a key that the view does not claim goes to it | uwui's own menu bar, drawn in the window |
| Cursor | `set_cursor`, `set_cursor_visible` (Android, iOS: none) | `View.setPointerIcon` (API 24) | CSS `cursor` | `UIPointerInteraction` | `NSCursor` | `Ui::cursor`, `Window::set_cursor` |
| Civil time | none: Linux `org.freedesktop.timedate1`'s `Timezone`; Windows `GetDynamicTimeZoneInformation`, `WM_TIMECHANGE`; macOS `NSSystemTimeZoneDidChange` | `java.time.ZonedDateTime`, `ACTION_TIMEZONE_CHANGED` | `Date`, `Intl.DateTimeFormat().resolvedOptions().timeZone` | `TimeZone.current`, `NSSystemTimeZoneDidChange` | `TimeZone.current`, `NSSystemTimeZoneDidChange` | none |

### Seams with track A

- One waker, the host's, given to uwui; the agents' sessions ask for a step through it.
- Meaning and actions stay in uwui's core. Each reader follows its own changes: `take_meaning_changes`
  stops emptying them for everyone.
- An element's capture is the host's image of the target, cropped to the element's place, which a
  question gives.

## Alternatives considered

The shape's alternatives are ADR-0248's. For the items:

- **Touch as its own event beside the pointer**, as winit 0.30 does: Pointer Events, Android's
  `MotionEvent` and winit 0.31 carry the kind with the pointer, and the stylus's pressure
  ([ADR-0190](0190-uwuis-requirements.md)) needs it.
- **The text around the caret mirrored in each host**, as Flutter's embedders keep it: each host
  would repeat uwui's rules of fields.
- **Offsets in UTF-16 in uwui**: uwui's text is UTF-8 (parley). Converting at the boundary counts
  from the start of the text, so its cost grows with the caret's place: 6 µs at 4,000 bytes.
- **Positions in logical pixels**, the web's, Apple's and Wayland's unit: the pointer's input, the
  hit test, the input method's area and the bounds of meaning made again in uwui, and Android,
  Windows and X11 dividing at the boundary, for an alignment on pixels that needs the scale anyway.
- **The clipboard read synchronously**: the web, Wayland and winit 0.31 read it asynchronously.

## Consequences

- uwui assumes one surface in places that targets change, which M6d.5c and M6d.5d take on: the
  hit test walks every root and a modal root stops every target; one pointer, hover, capture and
  focus; one display list for all roots; `Ui::cursor` and `Ui::text_input` name no target; layers
  sized by one window and dismissed by a press on any target; one tree of meaning over every
  root, where each window needs its own for AccessKit; `RootSettings` names no target. The
  prototype did not follow a layer to the root it stands over, so a menu over another target's
  root counted as the first target's: M6d.5c's tests hold that case.
- M6d.5b to M6d.5e build the items that their column names; the others wait for their reader, and
  this table is checked again when each is built.
- The web has no AccessKit adapter: ADR-0181's own, written in uwui, comes with the web's trial.
- winit 0.31 brings the pointer's kinds, surrounding text and an asynchronous clipboard; the move
  waits for `accesskit_winit` (ADR-0248).
- No version of winit stamps input: the engine and uwui's desktop host stamp events at reception
  (ADR-0227), and M6d.7 proposes winit a pull request after the open ones.
- Facts not confirmed in the reading of 2026-11-12, to confirm when their item is built: Windows'
  second start, files opened and high contrast; macOS's text's size; a surface lost on iOS; the
  character at a point on Android and the web; a rate asked on the web, Wayland and X11; low memory
  on the web; X11's input extension's times; whether winit reads `xdg_toplevel`'s `suspended`
  state; whether Windows tells a change of zone alone; forced colours anywhere but Windows and the
  web; windows of their own for layers on the web and iOS; a prompt on macOS's reads of the
  clipboard.
- winit 0.31's betas drop Android's mouse; `onLowMemory`, behind winit's `memory_warning` on
  Android, is never called from API 34: a host on Android reads `onTrimMemory` itself.

## Sources

Read on 2026-11-12.

- winit: [0.30.13's documentation](https://docs.rs/winit/0.30.13/winit/);
  [0.31.0-beta.4's changes](https://github.com/rust-windowing/winit/blob/v0.31.0-beta.4/winit/src/changelog/v0.31.md);
  its sources for [monitors](https://raw.githubusercontent.com/rust-windowing/winit/v0.30.13/src/monitor.rs), [X11's visibility](https://raw.githubusercontent.com/rust-windowing/winit/v0.30.13/src/platform_impl/linux/x11/event_processor.rs)
  and [Android](https://raw.githubusercontent.com/rust-windowing/winit/v0.30.13/src/platform_impl/android/mod.rs);
  [events' times, issue 1194](https://github.com/rust-windowing/winit/issues/1194) and
  [pull request 4562](https://github.com/rust-windowing/winit/pull/4562).
- AccessKit: [its repository](https://github.com/AccessKit/accesskit) and the crates
  [`accesskit`](https://docs.rs/accesskit/0.25.2/accesskit/),
  [`accesskit_winit`](https://docs.rs/accesskit_winit/0.34.2/accesskit_winit/),
  [`accesskit_android`](https://docs.rs/accesskit_android/latest/accesskit_android/),
  [`accesskit_ios`](https://docs.rs/accesskit_ios/latest/accesskit_ios/).
- Android: [`SurfaceHolder.Callback`](https://developer.android.com/reference/android/view/SurfaceHolder.Callback),
  [`MotionEvent`](https://developer.android.com/reference/android/view/MotionEvent),
  [`KeyEvent`](https://developer.android.com/reference/android/view/KeyEvent),
  [`InputConnection`](https://developer.android.com/reference/android/view/inputmethod/InputConnection),
  [`EditorInfo`](https://developer.android.com/reference/android/view/inputmethod/EditorInfo),
  [`InputMethodManager`](https://developer.android.com/reference/android/view/inputmethod/InputMethodManager),
  [`CursorAnchorInfo`](https://developer.android.com/reference/android/view/inputmethod/CursorAnchorInfo),
  [`WindowInsets`](https://developer.android.com/reference/android/view/WindowInsets),
  [`Activity`](https://developer.android.com/reference/android/app/Activity),
  [`ComponentCallbacks2`](https://developer.android.com/reference/android/content/ComponentCallbacks2),
  [`ClipboardManager`](https://developer.android.com/reference/android/content/ClipboardManager),
  [`Choreographer`](https://developer.android.com/reference/android/view/Choreographer),
  [`Choreographer.FrameTimeline`](https://developer.android.com/reference/android/view/Choreographer.FrameTimeline),
  [`Surface`](https://developer.android.com/reference/android/view/Surface),
  [`Configuration`](https://developer.android.com/reference/android/content/res/Configuration),
  [`UiModeManager`](https://developer.android.com/reference/android/app/UiModeManager),
  [`Settings.Global`](https://developer.android.com/reference/android/provider/Settings.Global),
  [`Handler`](https://developer.android.com/reference/android/os/Handler),
  [`InputDevice`](https://developer.android.com/reference/android/view/InputDevice),
  [`View`](https://developer.android.com/reference/android/view/View),
  [`Intent`](https://developer.android.com/reference/android/content/Intent),
  [`Menu`](https://developer.android.com/reference/android/view/Menu),
  [`PopupWindow`](https://developer.android.com/reference/android/widget/PopupWindow),
  [`View.OnDragListener`](https://developer.android.com/reference/android/view/View.OnDragListener),
  [`PopupMenu`](https://developer.android.com/reference/android/widget/PopupMenu),
  [the NDK's looper](https://developer.android.com/ndk/reference/group/looper),
  [`Display`](https://developer.android.com/reference/android/view/Display),
  [`SystemClock`](https://developer.android.com/reference/android/os/SystemClock),
  [`java.time.ZonedDateTime`](https://developer.android.com/reference/java/time/ZonedDateTime).
- The web (MDN): [Pointer events](https://developer.mozilla.org/en-US/docs/Web/API/PointerEvent),
  [`KeyboardEvent`](https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent),
  [`WheelEvent.deltaMode`](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent/deltaMode),
  [`Event.timeStamp`](https://developer.mozilla.org/en-US/docs/Web/API/Event/timeStamp),
  [`CompositionEvent`](https://developer.mozilla.org/en-US/docs/Web/API/CompositionEvent),
  [EditContext](https://developer.mozilla.org/en-US/docs/Web/API/EditContext_API),
  [`inputmode`](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/inputmode),
  [VirtualKeyboard](https://developer.mozilla.org/en-US/docs/Web/API/VirtualKeyboard_API),
  [the Clipboard API](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard_API),
  [`devicePixelContentBoxSize`](https://developer.mozilla.org/en-US/docs/Web/API/ResizeObserverEntry/devicePixelContentBoxSize),
  [`devicePixelRatio`](https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio),
  [`requestAnimationFrame`](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame),
  [`MessageChannel`](https://developer.mozilla.org/en-US/docs/Web/API/MessageChannel),
  [Page Visibility](https://developer.mozilla.org/en-US/docs/Web/API/Page_Visibility_API),
  [`pagehide`](https://developer.mozilla.org/en-US/docs/Web/API/Window/pagehide_event),
  [`webglcontextlost`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/webglcontextlost_event),
  [`GPUDevice.lost`](https://developer.mozilla.org/en-US/docs/Web/API/GPUDevice/lost),
  [`prefers-reduced-motion`](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/At-rules/@media/prefers-reduced-motion)
  and the other media features beside it,
  [`env()`](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Values/env),
  [the Gamepad API](https://developer.mozilla.org/en-US/docs/Web/API/Gamepad_API),
  [`contextmenu`](https://developer.mozilla.org/en-US/docs/Web/API/Element/contextmenu_event),
  [`cursor`](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/cursor),
  [`languagechange`](https://developer.mozilla.org/en-US/docs/Web/API/Window/languagechange_event),
  [the Launch Handler API](https://developer.mozilla.org/en-US/docs/Web/API/Launch_Handler_API),
  [`enterkeyhint`](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/enterkeyhint),
  [`MessagePort.postMessage`](https://developer.mozilla.org/en-US/docs/Web/API/MessagePort/postMessage),
  [`performance.now()`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/now),
  [`matchMedia`](https://developer.mozilla.org/en-US/docs/Web/API/Window/matchMedia),
  [`focus`](https://developer.mozilla.org/en-US/docs/Web/API/Window/focus_event),
  [`blur`](https://developer.mozilla.org/en-US/docs/Web/API/Window/blur_event),
  [`pageshow`](https://developer.mozilla.org/en-US/docs/Web/API/Window/pageshow_event),
  [`elementFromPoint`](https://developer.mozilla.org/en-US/docs/Web/API/Document/elementFromPoint),
  [`dragover`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/dragover_event), [`drop`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/drop_event),
  [`cut`](https://developer.mozilla.org/en-US/docs/Web/API/Element/cut_event), [`MutationObserver`](https://developer.mozilla.org/en-US/docs/Web/API/MutationObserver),
  [`setTimeout`](https://developer.mozilla.org/en-US/docs/Web/API/Window/setTimeout),
  [`Date`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date),
  [`Intl.DateTimeFormat.resolvedOptions`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_objects/Intl/DateTimeFormat/resolvedOptions).
- UIKit and Core Animation: [`UITextInput`](https://developer.apple.com/documentation/uikit/uitextinput),
  [`UIKeyInput`](https://developer.apple.com/documentation/uikit/uikeyinput),
  [`UITextInputTraits`](https://developer.apple.com/documentation/uikit/uitextinputtraits),
  [`UITouch`](https://developer.apple.com/documentation/uikit/uitouch),
  [`UIKey`](https://developer.apple.com/documentation/uikit/uikey),
  [`safeAreaInsets`](https://developer.apple.com/documentation/uikit/uiview/safeareainsets),
  [`UIKeyboardLayoutGuide`](https://developer.apple.com/documentation/uikit/uikeyboardlayoutguide),
  [`UISceneDelegate`](https://developer.apple.com/documentation/uikit/uiscenedelegate),
  [`UIApplicationDelegate`](https://developer.apple.com/documentation/uikit/uiapplicationdelegate),
  [`UIPasteboard`](https://developer.apple.com/documentation/uikit/uipasteboard),
  [`CADisplayLink`](https://developer.apple.com/documentation/quartzcore/cadisplaylink),
  [`CAFrameRateRange`](https://developer.apple.com/documentation/quartzcore/caframeraterange),
  [`UITraitCollection`](https://developer.apple.com/documentation/uikit/uitraitcollection),
  [`UIAccessibility`](https://developer.apple.com/documentation/uikit/uiaccessibility),
  [`GCController`](https://developer.apple.com/documentation/gamecontroller/gccontroller),
  [`UIPointerInteraction`](https://developer.apple.com/documentation/uikit/uipointerinteraction),
  [`UIMenuBuilder`](https://developer.apple.com/documentation/uikit/uimenubuilder),
  [`contentScaleFactor`](https://developer.apple.com/documentation/uikit/uiview/contentscalefactor),
  [`UIResponder`](https://developer.apple.com/documentation/uikit/uiresponder),
  [`UIResponderStandardEditActions`](https://developer.apple.com/documentation/uikit/uiresponderstandardeditactions),
  [`UIView.hitTest`](https://developer.apple.com/documentation/uikit/uiview/hittest(_:with:)),
  [`UIDropInteraction`](https://developer.apple.com/documentation/uikit/uidropinteraction),
  [`isDarkerSystemColorsEnabled`](https://developer.apple.com/documentation/uikit/uiaccessibility/isdarkersystemcolorsenabled),
  [`Timer`](https://developer.apple.com/documentation/foundation/timer), [`CFRunLoopTimer`](https://developer.apple.com/documentation/corefoundation/cfrunlooptimer),
  [`MTLDrawable.present(at:)`](https://developer.apple.com/documentation/metal/mtldrawable/present(at:)) and
  [`presentedTime`](https://developer.apple.com/documentation/metal/mtldrawable/presentedtime),
  [`UIPress`](https://developer.apple.com/documentation/uikit/uipress),
  [`UIContextMenuInteraction`](https://developer.apple.com/documentation/uikit/uicontextmenuinteraction),
  [`CAMetalLayer`](https://developer.apple.com/documentation/quartzcore/cametallayer),
  [`CFRunLoopSource`](https://developer.apple.com/documentation/corefoundation/cfrunloopsource),
  [`UIScreen.maximumFramesPerSecond`](https://developer.apple.com/documentation/uikit/uiscreen/maximumframespersecond),
  [`UIWindow.becomeKey`](https://developer.apple.com/documentation/uikit/uiwindow/becomekey()),
  [`CADisplayLink.targetTimestamp`](https://developer.apple.com/documentation/quartzcore/cadisplaylink/targettimestamp),
  [`preferredFrameRateRange`](https://developer.apple.com/documentation/quartzcore/cadisplaylink/preferredframeraterange),
  [`TimeZone.current`](https://developer.apple.com/documentation/foundation/timezone/current),
  [`NSSystemTimeZoneDidChange`](https://developer.apple.com/documentation/foundation/nsnotification/name-swift.struct/nssystemtimezonedidchange),
  [`mach_continuous_time`](https://developer.apple.com/documentation/kernel/1646199-mach_continuous_time).
- AppKit: [`NSTextInputClient`](https://developer.apple.com/documentation/appkit/nstextinputclient),
  [`NSEvent`](https://developer.apple.com/documentation/appkit/nsevent),
  [`NSView.safeAreaInsets`](https://developer.apple.com/documentation/appkit/nsview/safeareainsets),
  [`NSScreen`](https://developer.apple.com/documentation/appkit/nsscreen),
  [`NSApplicationDelegate`](https://developer.apple.com/documentation/appkit/nsapplicationdelegate),
  [`NSPasteboard`](https://developer.apple.com/documentation/appkit/nspasteboard),
  [`NSWorkspace`'s display options](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducemotion),
  [`effectiveAppearance`](https://developer.apple.com/documentation/appkit/nsapplication/effectiveappearance),
  [`NSMenu`](https://developer.apple.com/documentation/appkit/nsmenu),
  [`NSCursor`](https://developer.apple.com/documentation/appkit/nscursor),
  [`backingScaleFactor`](https://developer.apple.com/documentation/appkit/nswindow/backingscalefactor),
  `NSWindowDelegate`'s [`windowDidResize`](https://developer.apple.com/documentation/appkit/nswindowdelegate/windowdidresize(_:)),
  [`windowDidBecomeKey`](https://developer.apple.com/documentation/appkit/nswindowdelegate/windowdidbecomekey(_:)) and
  [`windowDidChangeOcclusionState`](https://developer.apple.com/documentation/appkit/nswindowdelegate/windowdidchangeocclusionstate(_:)),
  [`windowDidChangeBackingProperties`](https://developer.apple.com/documentation/appkit/nswindowdelegate/windowdidchangebackingproperties(_:)),
  [`NSApplication.mainMenu`](https://developer.apple.com/documentation/appkit/nsapplication/mainmenu),
  [`applicationDidHide`](https://developer.apple.com/documentation/appkit/nsapplicationdelegate/applicationdidhide(_:)),
  [`NSScreen.minimumRefreshInterval`](https://developer.apple.com/documentation/appkit/nsscreen/minimumrefreshinterval),
  [`NSTouch`](https://developer.apple.com/documentation/appkit/nstouch),
  [`NSWorkspace.didWakeNotification`](https://developer.apple.com/documentation/appkit/nsworkspace/didwakenotification),
  [`NSView.hitTest`](https://developer.apple.com/documentation/appkit/nsview/hittest(_:)),
  [`mouseDownCanMoveWindow`](https://developer.apple.com/documentation/appkit/nsview/mousedowncanmovewindow),
  [`NSDraggingDestination`](https://developer.apple.com/documentation/appkit/nsdraggingdestination),
  [`NSText.copy(_:)`](https://developer.apple.com/documentation/appkit/nstext/copy(_:)) and
  [`NSApplication.sendAction`](https://developer.apple.com/documentation/appkit/nsapplication/sendaction(_:to:from:)),
  [`windowWillClose`](https://developer.apple.com/documentation/appkit/nswindowdelegate/windowwillclose(_:)),
  [`addChildWindow`](https://developer.apple.com/documentation/appkit/nswindow/addchildwindow(_:ordered:)), [`NSPanel`](https://developer.apple.com/documentation/appkit/nspanel),
  [`NSTextContent`](https://developer.apple.com/documentation/appkit/nstextcontent),
  [the memory-pressure source](https://developer.apple.com/documentation/dispatch/dispatchsource/makememorypressuresource(eventmask:queue:)),
  [`MTLDevice.isRemovable`](https://developer.apple.com/documentation/metal/mtldevice/isremovable),
  [key events in Cocoa](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/HandlingKeyEvents/HandlingKeyEvents.html),
  [`accesskit_macos`](https://docs.rs/accesskit_macos/latest/accesskit_macos/).
- Windows: [`DwmGetCompositionTimingInfo`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmgetcompositiontiminginfo)
  and [`DWM_TIMING_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ns-dwmapi-dwm_timing_info),
  [`UISettings`](https://learn.microsoft.com/en-us/uwp/api/windows.ui.viewmanagement.uisettings),
  [the performance counter](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps),
  [`GetFrameStatistics`](https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgiswapchain-getframestatistics),
  the presentation manager's [`SetTargetTime`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/presentation/nf-presentation-ipresentationmanager-settargettime.md)
  and [`SetPreferredPresentDuration`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/presentation/nf-presentation-ipresentationmanager-setpreferredpresentduration.md),
  [`RegisterOcclusionStatusWindow`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/dxgi1_2/nf-dxgi1_2-idxgifactory2-registerocclusionstatuswindow.md),
  [`GetMessageTime`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessagetime),
  [`POINTER_INFO`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-pointer_info),
  TSF's [`GetText`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/textstor/nf-textstor-itextstoreacp-gettext.md),
  [`SetText`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/textstor/nf-textstor-itextstoreacp-settext.md) and
  [`GetACPFromPoint`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/textstor/nf-textstor-itextstoreacp-getacpfrompoint.md),
  [`IMR_DOCUMENTFEED`](https://learn.microsoft.com/en-us/windows/win32/intl/imr-documentfeed),
  [`WM_NCHITTEST`](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nchittest),
  [`WM_POWERBROADCAST`](https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast),
  [`CreateMemoryResourceNotification`](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-creatememoryresourcenotification),
  [`GetDynamicTimeZoneInformation`](https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/nf-timezoneapi-getdynamictimezoneinformation),
  [`WM_TIMECHANGE`](https://learn.microsoft.com/en-us/windows/win32/sysinfo/wm-timechange),
  [`SystemParametersInfo`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow),
  [`WM_SETTINGCHANGE`](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-settingchange).
- Wayland and X11: [the core protocol](https://gitlab.freedesktop.org/wayland/wayland/-/raw/main/protocol/wayland.xml),
  [`xdg-shell`](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/stable/xdg-shell/xdg-shell.xml),
  [commit timing](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/staging/commit-timing/commit-timing-v1.xml),
  [input timestamps](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/unstable/input-timestamps/input-timestamps-unstable-v1.xml),
  [text input v3](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/unstable/text-input/text-input-unstable-v3.xml);
  [X11's protocol](https://www.x.org/releases/current/doc/xproto/x11protocol.html),
  [Present](https://gitlab.freedesktop.org/xorg/proto/xorgproto/-/raw/master/presentproto.txt),
  [RandR](https://gitlab.freedesktop.org/xorg/proto/randrproto/-/raw/master/randrproto.txt),
  [Xlib's input methods](https://www.x.org/releases/current/doc/libX11/libX11/libX11.html).
- Linux: [logind](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.login1.html),
  [timedated](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.timedate1.html),
  [memory pressure](https://www.kernel.org/doc/html/latest/accounting/psi.html),
  [`clock_gettime`](https://man7.org/linux/man-pages/man2/clock_gettime.2.html),
  [Wayland's presentation time](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/stable/presentation-time/presentation-time.xml),
  [the portal's Settings](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.Settings.xml),
  [`org.freedesktop.Application`](https://specifications.freedesktop.org/desktop-entry/latest/dbus.html).
- Menus: [`muda`](https://github.com/tauri-apps/muda). Gamepads: [`gilrs`](https://github.com/Arvamer/gilrs).
