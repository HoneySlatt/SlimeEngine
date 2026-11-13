# 0255. The clipboard through the outputs, and the host's questions

- Status: Accepted; amends [0248](0248-the-hosts-api-cut-in-five-built-where-it-is-read.md) (what
  a copy writes travels in the outputs),
  [0249](0249-the-hosts-api-its-items-and-their-table.md) (the same; the surrounding text as an
  output and the kinds of a point wait for their readers; the row "Clipboard's rights" on macOS)
  and [0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md) (the
  answer's third state carries no text)
- Date: 2026-11-13

## Context

M6d.5c2's card ([`stage-6d.md`](../stage-6d.md#m6d5c2--the-clipboard-and-the-hosts-questions))
builds the clipboard and the host's questions on the API of
[ADR-0253](0253-input-as-the-hosts-events-each-targets-outputs-m6d5c-cut-in-three.md): copy, cut
and paste as events; numbered reads of the clipboard; the `Clipboard` trait gone; the deletion
around the caret; the text around the caret, a range's rectangle, the character at a point,
whether a point is uwui's.

The facts, read in the repository on 2026-11-13:

- uwui's fields read and wrote the system's clipboard through a trait, `uwui::tree::Clipboard`,
  that `slime-app` gave over `arboard`, read in the middle of a key's handling. The editor read
  it for its own paste and wrote the core's copy through `Ui::clipboard_text` and
  `Ui::set_clipboard_text`.
- winit 0.30 gives no edit commands, no deletion around the caret, and asks no question. No host
  reads the questions before M6d.7 and M6d.8.
- A field has no maximum length. Each edit lays its whole text out again, about 1.1 ms for 4,000
  bytes ([`measurements.md`](../measurements.md#m6d5a-the-hosts-api-tried-on-a-prototype)); undo
  keeps up to 100 whole copies of the text.

The platforms, read on 2026-11-13 (`.delegation/m6d5c2-web/`, local; sources below):

- On the web, a `copy` or `cut` handler must cancel the event for `clipboardData.setData` to be
  used; `clipboardData` is readable during `paste` alone.
- macOS 15.4 asks the person when a program reads the general pasteboard, unless the access is
  "both user originated and paste related" (`NSPasteboard.AccessBehavior`).
- Wayland's `delete_surrounding_text` counts bytes from the cursor, excluding the selection, from
  the preedit's ends while one is shown; winit 0.31's `Ime::DeleteSurrounding` asks the
  application to take the preedit out, delete, and put it back. Android counts Java chars, or code
  points, from the selection's ends, and clamps past the text's end; it deletes nothing where a
  code point's count meets a broken surrogate pair.
- Wayland's surrounding text holds at most 4,000 bytes and leaves the preedit out; Android's
  gives the selection whole; macOS counts its marked text in the document's places.
- macOS's `firstRect(forCharacterRange:actualRange:)` gives the first line's rectangle and the
  range it covers, to be asked again for the next lines; Windows' `GetTextExt` gives one bounding
  box; Android and the web's `EditContext` want a box for each character.

A challenge of a first design found the faults that the alternatives below record.

## Decision

The owner's decisions of 2026-11-13: what a copy writes travels in the outputs; a huge text is
pasted whole, its costs fixed later; the surrounding text as an output, and the kinds of a point,
wait for their readers. Then the lead's within them.

**What a copy writes travels in the outputs**, the owner's choice. `Outputs::clipboard_write`
holds the last text written since the host read: a field's copy or cut, by its keys or the
platform's `Copy` and `Cut`, and the program's own (`Ui::write_clipboard`). A host reads the
outputs after each event that the interface took, inside the platform's handler of that event: on
the web, the handler of `copy` reads them, writes `setData` and cancels the event. One home for
every write, and `Answer` stays a value of three states, copied freely.

**The answer's third state**, `Answer::TakenLeftToPlatform`: taken from the game, not prevented.
The keys of copy, cut and paste in a field answer it once the host told uwui that its platform
fires its own events from them (`Ui::set_clipboard_events`), as the web does; the host then gives
`Input::Copy`, `Input::Cut` and `Input::Paste`, which answer `Taken`. `Answer::taken` is true for
both.

**Reads asked through the outputs, answered as events.**

- A field's key of paste, without the platform's events, asks a read: a `ReadId`, kept with its
  field, told once in `Outputs::clipboard_reads`. The host writes before it reads, so that a read
  asked after a write reads it.
- `Input::ClipboardText { read, text }` answers it: taken when awaited, else not taken and
  nothing; `text` none when the clipboard holds no text, cannot be read, the person refused, or
  the host has none.
- A field's answer pastes if the field still holds the focus and did not lose it since: the focus
  leaving a field, by any path, its element removed included, drops every field's read.
- `Ui::read_clipboard` asks a read for the program and gives its `ReadId`; its answer reaches the
  program as `ActionKind::ClipboardText { read, text }`, without an element. It survives the
  focus's moves.
- At most 16 reads wait; past them the oldest is dropped, a program's answered at once with no
  text. A read that the host never answers waits until newer reads push it out.
- A second paste asks a second read; each answer pastes at the caret as it comes.
- A paste while an input method composes keeps what it composed as typed text, ends its input,
  the host told `InputMethod::On` again, then pastes after it. A press in the field and the
  program's selection end it the same way, so that the selection never leaves a composition.

**The `Clipboard` trait is gone**, with `Ui::set_clipboard`, `has_clipboard`, `clipboard_text`
and `set_clipboard_text`: one way to reach the clipboard, the host's. A cut deletes what it wrote
whether or not the host has a clipboard; the undo gives it back.

**The deletion around the caret**, `Input::DeleteAround { before, after, unit }`: `before` units
before the selection, or before the composition while one is shown, and `after` units after it.
The selection and the composition stay; a count past the text's end stops there; one that ends
inside a character deletes up to that character alone. One edit, which typing joins in one undo;
the composition is taken out first and put back, so that the undo gives back the text without it.
Android's count from the selection while composing is its host's to translate.

**Questions** on the field that holds the focus on a target, on `&mut Ui`, each answered from its
live state, its text laid out and scrolled as the next frame will show it:

- `Ui::text_around(target, TextQuery)`: up to `before` and `after` units around the selection,
  never cutting a character, the selection whole, and the composition whole when it counts; the
  excerpt,
  where it starts in the whole text, and the selection and the composition within it, as Wayland
  and Android count them.
- `Ui::range_box(target, range, unit)` and `Ui::character_at` count places in the field's text
  with the composition, as macOS does.
- `Ui::range_box(target, range, unit)`: the box of the range's first line, in the target's
  physical pixels, and where that line's part ends, as macOS asks; a host that wants each line, a
  bounding box or each character asks again. A range of one direction on one line is boxed from
  its ends' carets, whose cost does not grow with the line.
- `Ui::character_at(target, x, y, unit)`: where the character drawn there starts; none off every
  character, as macOS and Windows answer.
- `Ui::covers(target, x, y)`: whether a press there would be the interface's, for a click let
  through and a drop.

`TextUnit` names the unit, `Utf8`, `Utf16` or `CodePoint`; uwui converts at the boundary.

**What waits for its reader**, the owner's choice:

| Item | Its reader |
|---|---|
| The surrounding text as an output, told after each change | M6d.8, with winit 0.31's text input on Wayland |
| The kinds of a point, a bar's caption, edges and buttons | M6d.7's decorations |
| A paste without a field given to the program | a host whose platform gives it, the web's, macOS's through `paste:` |
| A maximum length of a field | the milestone that fixes a huge text's costs (below) |

**A huge text is pasted whole**, the owner's choice, as today; its costs are written down in
[`debt.md`](../debt.md) and fixed where they arise in a milestone of their own: undo by edits
rather than copies, the layout by paragraph, a large layout outside the cache's budget, and a
maximum length that the program sets, counted in graphemes, the cut told to the person.

**The engine.** `slime-app` shows the outputs after each frame, whether or not it drew an image,
and right after each event that the interface took: it writes the text on `arboard`'s clipboard,
opened as an interface's outputs are first shown, then reads it once and answers each read that
waits, none where the system gives no clipboard. A paste typed in a field lands in the step of its
key, which macOS is expected to let a program read without asking, a check by hand; the editor's
own paste is read after the frame that asked it, which macOS may ask about. A frame follows a read
answered. The editor's paste writes the core's last copy, asks a read, and
pastes the answer, or what the editor copied where the clipboard held none.

## Alternatives considered

- **The copy's text in the answer**, as ADR-0248, 0249 and 0253 read: the type would make the host
  write it inside its handler. But a field's copy would travel in the answer and the program's in
  the outputs, two homes for one thing, and `Answer` would hold a `String`, no longer copied
  freely.
- **A default maximum length**, 1 MB: bounded at once, but a limit that fits a frame would be about
  15 KB, too little for a field of several lines, and it leaves undo's copies and the cache's
  budget as they are. **A maximum set by the program, now**: no program reads it yet.
- **The surrounding text as an output now**: little code, but its shape follows winit 0.31's,
  which no host reads before M6d.7.
- **The outputs shown inside the frame's image**, as before: a window minimized or covered draws
  no image, so a copy waited for the window to come back, over what the person copied elsewhere,
  and the editor's paste waited for a frame that something else asked for.
- **A paste while composing dropped**, as the keys are then the input method's: the person's paste
  is not the input method's key, and the text composed is already shown.
- **A range's boxes, every line's and direction's**, as the prototype gave them: one walk of the
  line for each, about 0.1 ms over 4,000 bytes, which a box for each character of Android's or the
  web's multiplies. The first line's box and its end serve each platform's way.
- **Offsets from the whole text's start**, as the prototype gave them: five counts from the start
  where Wayland and Android want places in the excerpt.
- **The deletion widened to whole characters**: it deletes what was not asked for; neither
  platform widens.
- **The deletion in UTF-8 alone**, winit 0.31's unit: ADR-0249 names code points for Android, and
  the unit costs nothing beside the questions'.
- **A cut kept without a clipboard**, as the trait allowed: uwui does not know whether the host has
  one, and asking each host to say so adds a door for a case that the undo covers.

## Consequences

- `uwui::host` gains `TextUnit`, `ReadId`, `TextQuery`, `TextAround`, `RangeBox`, five inputs, the
  answer's third state and two outputs; `Ui` gains `set_clipboard_events`, `write_clipboard`,
  `read_clipboard` and the four questions; `ActionKind` gains `ClipboardText`.
  `host::Person::serve_clipboard` serves a test's clipboard.
- The editor's paste from its menu or its keys takes a frame more than before: its read is
  answered after the frame that asked it, and its request made at the next.
- The program's writes and reads go to the main target's outputs; M6d.5c3 says on which target
  they go once roots have targets.
- A question over a field of 4,000 bytes is measured against the prototype in
  [`measurements.md`](../measurements.md#m6d5c2-the-clipboard-and-the-hosts-questions); the
  lowest machine of the quality bar is not, none being set up.
- The clipboard checked by hand, on Linux and in the Windows virtual machine, joins the
  [checks by hand](../PLAN.md#checks-by-hand-waiting-for-the-owner); whether macOS asks on the
  engine's read in answer to ⌘V, in a field and for the editor's paste, in the macOS virtual
  machine.
- The compositors' series reads whether the engine opened the clipboard as before: it is opened
  as an interface's outputs are first shown, not at its first use.

## Sources

Read on 2026-11-13.

- Wayland: [text input v3](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/raw/main/unstable/text-input/text-input-unstable-v3.xml).
- winit: [0.31.0-beta.4's changes](https://github.com/rust-windowing/winit/blob/v0.31.0-beta.4/winit/src/changelog/v0.31.md),
  its `Ime` and `ImeSurroundingText` in the beta's sources.
- Android: [`InputConnection`](https://developer.android.com/reference/android/view/inputmethod/InputConnection),
  [`SurroundingText`](https://developer.android.com/reference/android/view/inputmethod/SurroundingText),
  [`CursorAnchorInfo`](https://developer.android.com/reference/android/view/inputmethod/CursorAnchorInfo),
  [`ClipboardManager`](https://developer.android.com/reference/android/content/ClipboardManager).
- Apple: [`NSTextInputClient`](https://developer.apple.com/documentation/appkit/nstextinputclient),
  [`NSPasteboard.AccessBehavior`](https://developer.apple.com/documentation/appkit/nspasteboard/accessbehavior-swift.enum),
  [`UITextInput`](https://developer.apple.com/documentation/uikit/uitextinput),
  [`UIPasteboard`](https://developer.apple.com/documentation/uikit/uipasteboard),
  [`UIPasteControl`](https://developer.apple.com/documentation/uikit/uipastecontrol).
- Windows: TSF's [`GetTextExt`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/textstor/nf-textstor-itextstoreacp-gettextext.md),
  [`SetText`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/textstor/nf-textstor-itextstoreacp-settext.md),
  [`GetACPFromPoint`](https://raw.githubusercontent.com/MicrosoftDocs/sdk-api/docs/sdk-api-src/content/textstor/nf-textstor-itextstoreacp-getacpfrompoint.md).
- The web: [the Clipboard API and events](https://w3c.github.io/clipboard-apis/),
  [MDN's `copy` event](https://developer.mozilla.org/en-US/docs/Web/API/Element/copy_event),
  [EditContext](https://w3c.github.io/edit-context/).
