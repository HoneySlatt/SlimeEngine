# 0158. The viewport

- Status: Accepted; amends [0155](0155-the-editors-design.md) (the `.keys` preset in Blender's way
  comes with M6c.2b's picking, and an agent frames the viewport by an operation's name rather than
  flying it with scripted keys), [0127](0127-the-settings-of-a-view.md) (a view says when its new
  size has held still) and [0041](0041-render-graph.md) (sixteen passes to a submission, where it
  kept four)
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided the viewport: a view of its own in the editor's
window, drawn again only when what it shows changed, and once more `RESIZE_DELAY` after its size
last changed; `Kind::Surface`, an element kind that gives the program the pointer; navigation as
most editors have it, the cursor held meanwhile; the grid and icons from Lucide's glyphs; picking,
the outline and modes of debugging. Milestone M6c.2 was cut in two at its start: M6c.2a, the
viewport and its navigation, which this ADR records; M6c.2b, picking and the selection.

What the code held before this milestone:

- `slime-render` drew views into targets or rectangles of them, with what a client draws besides,
  lines and instances in the scene and lines over the image (ADR-0129). A view kept its targets at
  their old size until a frame drawn `RESIZE_DELAY` after its size last changed (ADR-0127), which
  nothing told a program that draws only when something changes.
- `slime-ui` took every press and every turn of the wheel over what it draws: no element gave the
  program the pointer, and a key reached a field or a shortcut before the program.
- The `world` example drew a grid and the icons of its lights with code of its own, and turned no
  camera with the mouse; nothing in the engine did.
- The editor's window held a region with a word in its center (ADR-0156). Its program knew the
  game's types (`Program::register`), but not how the game lights its levels, which the game's code
  chooses.
- Tools drive a running program's keys through the simulation's input (`Request::Press`), which a
  presentation's events do not see.

## Decision

**A surface.** `Kind::Surface` (`Element::surface`) is an element that shows an image that the
program draws and gives the program the pointer: a press of any button over it, the pointer's moves
while a button pressed over it is held, its release wherever it is, and the wheel over it, as
`ActionKind::Surface(SurfaceInput)`, in logical pixels from its corner, which the interface takes.
A button pressed over it holds the pointer until it is let go, any other button pressed meanwhile
going to it too; the press takes the focus from a field. While a surface holds the pointer, the keys
without Control, Alt or Super reach the program: no field types them and no shortcut runs, so that
the keys that fly fly, wherever the focus is. Screen readers and agents read it as an image, named
by its builder.

**The viewport.** The editor's window shows the level in its center, through a surface whose image
is a render target of the viewport's own, at the surface's size in pixels, which the canvas draws
as the interface's other images (`CanvasImage::Target`). The viewport holds its own asset cache,
which loads from the cooked data that the core opened, its own frame builder and its own view, on
the window's GPU device; it is made once a level is open, and again after a lost device. Its image
is drawn again only when what it shows changed: the world's tick, the camera, the image's size, its
settings, a mesh that came, or a new size that has held still, which the view now says
(`View::resize_due`) and at which the window asks for a frame (`Context::request_frame_at`, a frame
at a time, beside `Context::request_frame`). Between those, the window's frames show the image that
it holds. Tools capture it and change its settings by the name `viewport`, and read in the report
`viewport` how many times it drew, which an editor left alone does not change.

**The light is the game's.** How a game lights its levels, its sky, its exposure and its bloom, is
its code's, which the editor does not run. `Program::lighting` gives it: *Hunna Wood*'s editor gives
the game's day, `hunnawood::OVERCAST`, `DAY_EXPOSURE` and `BLOOM`; the `editor` example the
sample's. Which of a level's lights shine, a game's rules choose, as *Hunna Wood*'s hide the night's
by day: the viewport, which runs none, draws every light that the level places, which a level of
two times of day shows both of (known debt).

**Navigation.** The right button held flies: the mouse's raw motion turns the camera where it
stands, W, A, S, D move it forward, back and sideways, Q and E down and up, Shift four times faster,
and the wheel changes the speed. Alt and the left button orbit around the point that the camera
looks at; the middle button pans the camera and that point together; the wheel alone comes nearer
or goes farther. While a gesture lasts, the window holds the cursor, hidden, and the raw motion
turns or moves the camera ([ADR-0157](0157-dialogs-the-cursor-held-and-the-gpus-identity.md));
where the windowing system refuses, the cursor stays free, the pointer's moves over the surface
turning or moving the camera in place of the raw motion, and the user is told once. F,
`view.frame_selection` in the View menu, frames what is selected and what lies below it, or the
whole level when nothing is; the level is framed so as it opens, once its meshes have loaded.

**What the viewport draws besides the level.** A grid on the ground, a line a metre around the
point that the camera looks at, as far as 50 m, and the icons of the lights, Lucide's lightbulb,
flashlight and sun in white on a dark disc, unlit and blended, above each light, facing the camera.
The world has no cameras yet, whose icons wait for them. `slime-render` gains the grid's lines
(`Line::grid`) and the transform of a square that faces the camera (`facing`), which the `world`
example's grid and icons now use too.

**Agents.** Scripted keys reach the simulation's input, not the window: an agent does not fly the
viewport. It captures it, changes its settings, and frames what it selected by the operation's
name, which moves the camera as F does.

**Blender's way waits for picking.** A preset in Blender's way changes the mouse's buttons, the
middle one orbiting and the left one selecting, which picking brings: it comes with M6c.2b.

**Sixteen passes to a submission.** `PASSES_PER_SUBMISSION` and the bloom were measured on the
owner's machine while the desktop held the GPU's memory that the CPU maps whole
(`docs/measurements.md`, M6c.2a): a frame's CPU grows with its submissions, the bloom's many small
passes the most; a whole frame in one submission cost least, 3.5 to 4.7 ms against 4.2 to 5.3 ms
at four passes, without the occasional 8 ms in the kernel that ADR-0041 kept four passes against.
The owner, who chose four, chose sixteen on 2026-10-28, a whole frame of the samples in one
submission; Direct3D 12 is checked in the Windows virtual machine at a checkpoint.

## Alternatives considered

- **The view drawn into the window's image, under the interface**: the window's image does not
  persist from one frame to the next, so the view would be drawn at every frame that the interface
  draws, a hover or a caret, at several milliseconds in the stall that ADR-0155 measured.
- **The camera turned by the pointer's moves over the surface** rather than by the raw motion: the
  cursor would meet the screen's edges and stop the camera, and on Wayland a locked pointer reports
  no moves at all.
- **The editor's own sky and exposure, the same for every game**: a level would show in the editor
  otherwise than in its game, which a level's light is made for.
- **A wake per load that ends** rather than a frame at each frame while meshes load: the uploader
  copies what loads bring within a budget at each frame (ADR-0118), which frames drive anyway; the
  frames stop once nothing loads.
- **The keys of a tool's script given to the window's events**: the simulation's input is what a
  game's agent drives; a second path for the editor's would give the same keys two meanings.
- **The editor stretching its image until its new size held still**, rather than the view: two
  rules for one delay, which ADR-0127 already gives the view.
- **Eight passes to a submission**, two submissions a frame: nearly the same gain, kept in case a
  driver stalled again on whole frames, which nothing measured shows.
- **Keys suspended only while the right button flies**, as ADR-0155 said: any gesture over the
  surface holds the pointer, and a person who orbits does not type either; one rule is simpler.

## Consequences

- An idle editor's viewport draws nothing; a test of an agent's session checks that a menu's frames
  leave it undrawn and that framing draws it.
- A program of the editor gives its lighting; a game's editor package gives the game's.
- `slime-ui` programs have a way to draw what they own, under the pointer, inside an interface.
- The cursor held, the raw motion and the gestures are checked by hand on niri, Hyprland and X11
  through XWayland.
- M6c.2b adds picking with the left button, the marquee, the outline, the modes of debugging, a
  second viewport and Blender's way.
