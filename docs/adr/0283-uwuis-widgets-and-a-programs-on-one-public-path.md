# 0283. uwui's widgets and a program's on one public path

- Status: Accepted; superseded in part by
  [0294](0294-the-elements-contract-designed-by-e1-before-m6d10-on-a-prototype-thrown-away.md)
  (when and how the public path is designed)
- Date: 2026-11-16

## Context

An element of uwui's tree is one of the kinds that `uwui::tree::Kind` lists, 28 on 2026-11-16
([ADR-0140](0140-the-tree-of-elements.md), [ADR-0148](0148-widgets.md)). Each kind's layout,
drawing, input and meaning are uwui's private code: `Kind` is named 372 times in 20 files, and
uwui's public traits are sealed.

- A program that needs a widget that uwui lacks, an editor of nodes or a timeline, cannot write it
  as uwui writes its own: it forks uwui, or draws it in a surface.
- A surface (`Kind::Surface`, [ADR-0158](0158-the-viewport.md)) shows the program's image and gives
  it the pointer. It may declare its role and name (`Element::role`, `Element::name`), and reads as
  an image by default; what a screen reader or an agent may do on it, its actions, comes from its
  kind alone and cannot be declared.
- [ADR-0222](0222-what-a-theme-is.md) plans a theme's parts, states and slots as API. uwui's first
  stage builds its authoring API, then its library of components
  ([ADR-0194](0194-stage-6d-uwui-leaving-the-engine.md)).
- Frameworks that last write their own components on the path that they give others: Flutter's
  widgets, GPUI's elements in Zed, Masonry's widgets.

## Decision

The owner's decision of 2026-11-16.

**What a widget of uwui's can do, a program's can, through the public API:** be measured and laid
out, draw, take input and the focus, give its meaning to screen readers and agents, its role,
name, value and actions, and carry a theme's parts and states.

- **Designed with the authoring API,** in uwui's first stage, before its library of components;
  M6d.10 places the milestone. The ADR that designs it supersedes what of ADR-0140 and ADR-0148
  it changes.
- **uwui's own widgets then move onto that path,** so that 28 widgets try it before uwui's 1.0
  freezes it.
- **A faster path inside uwui stays allowed,** measured, as long as it gives uwui's widgets no
  capability that a program's lacks.
- **Until then, a surface declares its actions,** as it declares its role and name, so that what a
  program draws in one can be read and used by screen readers and agents; placed by M6d.10 among
  uwui's first stage's milestones, or taken earlier by a milestone that works on surfaces.

## Alternatives considered

- **Widgets composed of uwui's elements, and a surface that declares its meaning, alone:** far
  cheaper, and enough for most programs; but a behaviour of its own, a focus and a selection inside
  a widget, written again in each surface, and uwui's widgets keeping what others lack.
- **Widgets added inside uwui alone:** no work, but a framework meant for others after its 1.0
  that they cannot extend without a fork.

## Consequences

- uwui's first stage holds a design whose cost is not yet known; it is weighed at M6d.10.
- The tree's closed list of kinds gives way, at that stage, to whatever the design chooses; the
  engine's editor, which builds on uwui, follows at its next port
  ([ADR-0277](0277-the-engine-ported-to-uwui-at-the-end-of-each-of-uwuis-stages.md)).
- The budgets of uwui's frames and lists hold across the change: the design is measured against
  them.

## Validation and reconsideration

Counted: `Kind`'s kinds and the files that name it. Read: how a surface's role, name and actions
are found (`tree/meaning.rs`). Assumed: that one path can meet uwui's budgets.

Proposing another decision is justified if the design at uwui's first stage cannot meet the
budgets of frames and lists on one path.
