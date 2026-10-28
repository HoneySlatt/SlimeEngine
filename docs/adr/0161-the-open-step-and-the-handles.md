# 0161. The open step and the handles

- Status: Accepted; amends [0155](0155-the-editors-design.md) (snapping moves the pivot onto the
  world's grid in the world's axes, by increments in the selection's alone, and an operation turns
  it on and off), [0135](0135-the-editors-program.md) (a tool's requests that change an editor's
  level or its selection wait while a person's gesture is under way) and
  [0129](0129-drawing-for-the-editor.md) (what a client draws over the image gains unlit instances,
  with a depth of their own)
- Date: 2026-10-28

## Context

[ADR-0155](0155-the-editors-design.md) decided how the user moves what is selected: a gesture is
one step of the history, through an *open step* of the core (its decision D3), the requests of
agents waiting meanwhile; handles of the engine's own, hit by the CPU, drawn as unlit instances over
the image with a depth of their own, mapped onto the world's uniform scale and its stretch;
snapping by increments and to an absolute grid; End dropping the selection on what lies below, by
the CPU's picker, without physics. Milestone M6c.3, which was to bring them with creating and
placing, was cut in two at its start, on 2026-10-28, as ADR-0155 allows: M6c.3a, the open step,
the handles and the drops, which this ADR records; M6c.3b, creating, duplicating, placing prefabs
and the history panel.

What the code held before this milestone:

- The editor's core ([ADR-0134](0134-the-editors-core.md)) made each change a step as it came,
  journaled at once; nothing changed the level without being one.
- The development channel answered a tool's `Edit` request at the start of the next frame
  ([ADR-0135](0135-the-editors-program.md)), whatever the person was doing.
- What a client draws over a view's image (`Drawings::over_image`) was lines alone, drawn without
  depth; [ADR-0129](0129-drawing-for-the-editor.md) left instances there for the handles.
- `slime-world-render`'s `Picker` cast rays by the CPU, the nearest mesh first or every mesh met
  ([ADR-0159](0159-picking-and-the-selection.md)), and gave no entity's bounds.

## Decision

**The open step.** `Editor::gesture(world, commands)` applies commands live, after undoing those
that the gesture applied before, so that each frame of a drag sends where the selection is now,
not by how much it moved since the last frame; they are neither journaled nor in the history.
`Editor::end_gesture(world, label)` undoes them and makes them one step, the last commands sent,
journaled as any step; `Editor::cancel_gesture` undoes them and closes the gesture.
Commands refused end nothing: the level keeps the gesture's last commands that were applied. While
a gesture is open, every other change of the level, a step, an undo, a redo or a writing, is
refused (`EditError::Gesture`), so that nothing interleaves with it; a crash loses the gesture
alone, which the journal never held. The window sends the core three requests more,
`Request::Gesture`, `EndGesture` and `CancelGesture`, through the value they share, as ADR-0155's
D1 says.

**A tool's edits wait.** `Simulation::edits_wait`, a method of the development build, says whether
what a tool asks that changes an editor's level or its selection waits; the editor's program says
so while a gesture is open, and from the press that opens one, before the core has answered it, so
that no step slips between the press and the first move. The development channel then keeps each
such request, an `Edit` request but those that read the history, the journal or the operations,
and a value set, which an editor makes a step of, in its order, the next ones behind those that
wait already, asks frames meanwhile, and answers them at the first frame at which they no longer
wait; what a tool reads, the world, the history, a capture, is answered at once and shows the
gesture as it is applied. A tool waits as long as the person holds the button; the engine's MCP
server asks one request at a time, so that its other tools wait behind it. The channel has no open
step, as ADR-0155 decided. The platform lets go of a button held when the window loses the focus,
so that every gesture ends.

**The handles.** *Edit* > *Move*, *Turn* and *Scale* (`edit.handles_move`, `edit.handles_turn`,
`edit.handles_scale`, W, E and R) choose the handles of what is selected, the one in use ticked;
*Handles in the selection's axes* (`edit.handles_local`) turns them with the last entity
selected, in the world's axes otherwise; *Snap* (`edit.snap`) snaps what they do. They stand at
the middle of the origins of what they move, each viewport drawing them 100 logical pixels long
whatever the camera's distance: an arrow along each axis and a square across each pair of axes, to move along
it or in its plane; a ring about each axis, to turn; a stick ending in a cube along each axis and a
cube in the middle, to scale. The axes are red, green and blue, the part dragged yellow, the middle
white. They move, turn and scale the outermost of what is selected, each entity without an
ancestor in the selection, which carries its children, so that none moves twice. The left button,
or whichever the table of shortcuts binds to `select`
([ADR-0160](0160-two-viewports-and-blenders-way.md)), pressed over a handle starts a drag in
place of a click, and opens the gesture: the CPU finds the part that the ray through the pointer
meets first, at once. At each move, the window sends the commands that put each entity where the
drag takes it, each setting its whole `Transform`: moved by the drag's distance along its axis or
plane; turned about the axis through the pivot; scaled along an axis, its stretch along its own
axis nearest that one, which no child inherits, by how much farther from the pivot the pointer
is; scaled from the middle, its uniform scale, and its distance from the pivot with it, by how far
the pointer moved away from the pivot, the handles' length doubling it. Where the pointer's ray
reaches nothing that the part moves along, as along the axis itself, what is dragged stays where
it was last taken. Letting go makes the step, named *Move*, *Turn* or *Scale*, none if nothing
moved; Escape cancels it. Snapping keeps to the axis or the plane moved: in the world's axes, it
lands the pivot on the world's grid of a quarter of a metre along them, which no grid lies along in
the selection's, where it moves by steps of a quarter of a metre from where the drag started; it
turns by fifteen degrees, and scales by tenths.

**Drops.** *Drop on what lies below* (`edit.drop`, End) moves each of the outermost of what is
selected straight down until its origin meets the first mesh below it, by the CPU's ray through
every mesh (`Picker::cast_through`), its own and its children's left out; *Drop its bottom on what
lies below* (`edit.drop_bottom`, Shift+End) until the lowest point of its meshes' box in the world,
which `Picker::bounds` gives, meets it. Both are one step, made at once with or without a window,
and refused when nothing lies below anything selected: the CPU's picker meets meshes, not
colliders, nor the ground's grid that the viewport draws.

**Instances over the image.** `Drawings::over_image` becomes a `DrawList`, lines and instances. Its
instances of the unlit kind are drawn over the view's image after the tone mapping, in the output's
colors, culled and placed as those in the scene are, with a depth target of their own, of the
output's size, which the view makes with its first frame that draws there: the nearest hides the
farthest among them, and nothing of the scene hides them; those of another kind are left out. A
pass of its own clears that depth before, as a pass that keeps the image clears no depth target;
the lines are drawn after the instances, over them. The unlit kind gains six pipelines for them,
opaque and blended, one for each culling. `RenderStats::over_image_gpu_time` counts both passes.

## Alternatives considered

- **Each frame of a drag sending what moved since the last one**: rounding builds up, and snapping
  needs where the drag started; the whole transform from where it started costs as little.
- **A gesture that lets other steps through**: an undo during a drag would undo what the gesture
  stands on; refused instead, the person sees why.
- **A tool's edits refused during a gesture**, or applied into it: refused, an agent retries
  blindly; applied, the gesture's undo would undo the agent's change too.
- **Every request of a tool waiting during a gesture**, its reads too: an agent that reads the
  history while a person drags learns nothing more by waiting.
- **Handles drawn by the window's interface**, shapes projected by the CPU over the viewport: no
  depth among the parts, and a second way of drawing what the viewport draws already.
- **The handles hidden by the scene**, drawn in it with its depth: a handle inside a wall cannot
  be reached.
- **The depth cleared by the pass that draws over the image**: a pass that keeps its color and
  clears its depth, which the render graph does not declare.
- **Snapping held under Ctrl**, as some editors have it: the pointer's moves carry no modifiers in
  `slime-ui`'s surface input; a ticked operation reaches agents and the palette too, and a key can
  be bound to it.
- **Snapping by increments of distance from where a drag starts, in the world's axes too**: what
  starts off the grid stays off it; the level's tiles line up on the world's grid.
- **The scale from the middle by the ratio of the pointer's distances from the pivot**, as along an
  axis: a press near the pivot divides by almost nothing, and the smallest move scales many times.
- **Each entity scaled about its own origin**: a group scaled from its middle would come apart.
- **Scaling along an axis by the uniform scale**: the world has no scale per axis but the stretch
  ([ADR-0083](0083-transforms-and-the-hierarchy.md)).
- **Drops by the GPU's picking**: answered a frame or two later, and not without a window, where
  an agent's editor runs.
- **Drops by physics**: the engine has none yet.

## Consequences

- The open step is the core's; the sliders and the fields of the inspector, whose drags ADR-0155
  makes one step each too, can use it when they change the level.
- A tool waits while a person drags; the history it reads afterwards holds the person's step before
  its own.
- A view that draws instances or lines over its image holds a depth target as large as its output,
  some 32 MiB at 3840 × 2160, and runs two passes more at each frame that draws there.
- A drag of a large selection sends a command for each entity at each move, which the core undoes
  and applies again: nothing has measured it beyond a few entities.
- The pivot is the middle of the origins, not of the bounds: a mesh whose origin lies at its base
  turns about its base.
- M6c.3b brings creating, duplicating, copying, grouping, placing prefabs and the history panel.
