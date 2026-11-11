# 0171. Effects of the interface, at their lowest cost

- Status: Accepted; amends [0170](0170-applications-on-the-ui-framework.md) (Stage 6d lays the
  foundations of the interface's effects, and measures the power that it draws); amended by
  [0190](0190-uwuis-requirements.md) (motion interrupted and driven by gestures, shared elements; effects lightened on
  battery) and [0194](0194-stage-6d-uwui-leaving-the-engine.md) (the foundations built in uwui's first
  stage), then by [0227](0227-the-time-that-motion-reads.md) (each animation's cap a whole divisor of the refresh, halved for lasting
  motion while saving power) and [0235](0235-content-from-outside-the-tree.md) (glass over a plane that uwui cannot read falls back,
  to the system's glass among other steps on Apple's platforms)
- Date: 2026-11-04

## Context

[ADR-0170](0170-applications-on-the-ui-framework.md) made applications the third client of the
engine's UI framework, and inserted Stage 6d, which gives the framework a shell that runs an
application, transforms, masks, richer drawing and motion. Right after, the owner stated what
these applications, and others to come, are to look like: motion as refined as current interfaces
have it, shapes that morph into one another on springs, fonts whose weight moves, glass that
blurs and bends what lies behind it, as aesthetic and detailed as can be, always at the lowest
draw of power.

What these looks need, beyond what ADR-0170 listed:

- **Groups drawn apart.** A part of the tree drawn into an image of its own, then laid over the
  rest: what masks need, the opacity of a group that overlaps itself, and every effect below.
- **What lies behind an element.** Glass reads what is drawn below it, blurs it, bends it along
  its edges, tints it after the light of what lies beneath, and adds a highlight; shapes of glass
  that meet merge as drops do.
- **Vector shapes that morph.** A button or an indicator turns from one shape into another; boxes
  with rounded corners cannot draw that.
- **Fonts whose axes move.** A variable font's weight or width animated, its glyphs rasterized
  again as they change.
- **An element that becomes another**, a card growing into the page that it opens.

The canvas draws today every item of an interface in one pass, with one pipeline
([ADR-0137](0137-the-canvas.md)). The GPU layer gives render passes; compute shaders come with
their first user ([ADR-0115](0115-what-the-gpu-layer-gives-the-editor.md)). The renderer's bloom
already blurs on a chain of images, each half the size of the one before.

## Decision

The owner's decision of 2026-11-04.

**The foundations in Stage 6d.** Stage 6d, which redraws the canvas for transforms, masks and
gradients, makes it compose groups as it does so, rather than once now and again later for
effects. It lays, for games, the editor and applications alike:

- groups drawn apart into images of their own, kept from one frame to the next and drawn again
  only when what they hold changes, then composed;
- what lies behind an element, for glass: blurred on reduced images, as the bloom blurs, bent
  along the element's edges, tinted, lit by a highlight;
- vector shapes that morph into one another, drawn smoothly by the GPU; how they are drawn is
  decided at the stage's start, by measurement;
- variable fonts whose axes are animated;
- an element that becomes another;
- a first material of glass, shown in an example.

These go through render passes, as the bloom does. A measurement that asks for a compute shader
makes this stage its first user, as ADR-0115 allows.

**Paying only for what shows and changes.** Nothing is drawn at rest; a group is composed from
its kept image while it does not change; a blur works on reduced images; effects stop while the
window has lost the focus or is hidden; each animation's frame rate is capped. A person lightens
them: reduced motion, which Stage 6b brought ([ADR-0149](0149-the-looks-foundations.md)), and
reduced transparency, which this stage brings; the effects lighten on the modest and handheld
tiers of hardware ([ADR-0114](0114-hardware-tiers-and-the-gpus-limits.md)).

**Power, measured.** The power that the GPU and the CPU draw joins the stage's exit criteria,
beside its times and its memory: each effect at rest and in motion, on the owner's machine and on a
modest one. How it is measured is decided at the stage's start.

**Glass bends what the application draws.** A program cannot read the windows of others under
Wayland: glass over what lies behind the window, the desktop, is the compositor's to draw, and
outside the framework.

## Alternatives considered

- **Effects in a stage after Stage 6d**: the applications ported sooner, but the canvas rewritten
  a second time to compose groups, and the applications ported once without the looks that they
  are made for.
- **A renderer of applications of their own**, Vello or Skia: two ways to draw an interface, the
  one that [ADR-0132](0132-the-engines-ui-framework.md) refused; Vello needs compute shaders
  throughout, and Skia is a library in C++.
- **Glass drawn by the compositor**, through the protocols by which some compositors blur what
  lies behind a window: only behind the window, never within it, and only where the compositor
  offers it.

## Consequences

- Stage 6d grows longer: the applications' ports wait for it, as the owner chose.
- The canvas goes from one pass to groups composed, which the editor's panels and the games'
  menus use too.
- Every effect carries a cost measured in power, not only in time, and a way to lighten it.
- Tests of effects run on lavapipe in CI, on the CPU, and take longer there.
