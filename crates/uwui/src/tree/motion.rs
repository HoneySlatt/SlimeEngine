//! Transitions (`docs/adr/0149-the-looks-foundations.md`): an element's colors and opacity going
//! from what they showed to what its new style gives, and a section growing or shrinking to its
//! height as it opens or folds, over the time that the program gives the interface each frame
//! ([`Ui::advance`]), along the style's [`Easing`](crate::style::Easing).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use slime_color::LinearColor;

use super::{ElementId, Kind, Ui};
use crate::style::{Colors, Style};

/// The number of colors that a transition moves: every color of a style, as the derive lists them.
const COLORS: usize = <Style as Colors>::COUNT;

/// What a transition moves: a style's colors, those that the derive lists (`Colors`), and its
/// opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Shown {
    colors: [LinearColor; COLORS],
    opacity: f32,
}

impl Shown {
    /// What `style` shows of what a transition moves.
    fn of(style: &Style) -> Self {
        let mut colors = [LinearColor::rgba(0.0, 0.0, 0.0, 0.0); COLORS];
        let mut place = 0;
        style.read(&mut |color| {
            colors[place] = color;
            place += 1;
        });
        Self {
            colors,
            opacity: style.paint.opacity,
        }
    }

    /// `style`, showing these colors and this opacity.
    fn applied(self, style: &Style) -> Style {
        let mut style = style.clone();
        let mut colors = self.colors.into_iter();
        style.write(&mut || colors.next().expect("as many colors as the style holds"));
        style.paint.opacity = self.opacity;
        style
    }

    /// What shows `share` of the way from these to `to`: these exactly at the start, so that the
    /// frame that starts a transition shows what showed, a transparent color's channels kept.
    fn toward(self, to: Self, share: f32) -> Self {
        if share <= 0.0 {
            return self;
        }
        Self {
            colors: std::array::from_fn(|place| {
                between(self.colors[place], to.colors[place], share)
            }),
            opacity: self.opacity + (to.opacity - self.opacity) * share,
        }
    }

    /// Its colors' channels and its opacity by their bits, which tell two apart as `Same` does.
    fn bits(self) -> [u32; COLORS * 4 + 1] {
        let mut bits = [0; COLORS * 4 + 1];
        for (place, color) in self.colors.iter().enumerate() {
            for (channel, value) in [color.r, color.g, color.b, color.a].into_iter().enumerate() {
                bits[place * 4 + channel] = value.to_bits();
            }
        }
        bits[COLORS * 4] = self.opacity.to_bits();
        bits
    }
}

/// The styles that transitions show at one frame, each made once for the elements that move alike:
/// from the same colors shown, towards the same style, started at the same time
/// (`docs/adr/0232-transitions-read-the-derives-colours-and-share-what-they-show.md`). Emptied at
/// each frame, its room kept.
#[derive(Default)]
pub(crate) struct ShownStyles {
    /// Each style shown, with the target that it was made for, kept alive until the map is emptied
    /// so that no other style takes its place in memory, and so its key, meanwhile.
    made: HashMap<Alike, (Arc<Style>, Arc<Style>)>,
}

/// What elements that move alike share: the style that they go to, by its place, the colors that
/// they started from, by their bits, and when they started.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Alike {
    target: usize,
    from: [u32; COLORS * 4 + 1],
    started: Duration,
}

impl ShownStyles {
    /// The style that shows `share` of `motion`, made once for the elements that move alike.
    fn get(&mut self, motion: &Motion, share: f32) -> Arc<Style> {
        let alike = Alike {
            // The style's place, which the map keeps alive with it.
            target: Arc::as_ptr(&motion.target) as usize,
            from: motion.from.bits(),
            started: motion.started,
        };
        let (_, made) = self.made.entry(alike).or_insert_with(|| {
            let shown = motion.from.toward(Shown::of(&motion.target), share);
            (
                Arc::clone(&motion.target),
                Arc::new(shown.applied(&motion.target)),
            )
        });
        Arc::clone(made)
    }

    /// Forgets the styles of the frame before.
    pub(crate) fn clear(&mut self) {
        self.made.clear();
    }
}

/// The color `share` of the way from `from` to `to`, premultiplied by alpha, so that a color that
/// fades from transparent shows its own hue all the way, not the hue of the transparent black.
fn between(from: LinearColor, to: LinearColor, share: f32) -> LinearColor {
    let alpha = from.a + (to.a - from.a) * share;
    if alpha <= 0.0 {
        return LinearColor::rgba(0.0, 0.0, 0.0, 0.0);
    }
    let channel = |a: f32, b: f32| (a * from.a + (b * to.a - a * from.a) * share) / alpha;
    LinearColor::rgba(
        channel(from.r, to.r),
        channel(from.g, to.g),
        channel(from.b, to.b),
        alpha,
    )
}

/// A transition of an element's colors and opacity under way.
pub(crate) struct Motion {
    /// The style that it goes to, which the rules and the element gave.
    pub(crate) target: Arc<Style>,
    /// What showed when it started.
    from: Shown,
    /// When it started, on the interface's clock.
    started: Duration,
}

/// A section growing or shrinking to its height.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reveal {
    /// Its height when it started, in pixels of the screen.
    from: f32,
    /// The height that it goes to: the height that it takes open, once a layout has measured it,
    /// or folded.
    pub(crate) to: Option<f32>,
    /// Whether it opens.
    pub(crate) opening: bool,
    /// When it started, on the interface's clock.
    started: Duration,
}

impl Reveal {
    /// Its height now, in pixels of the screen; none until its open height is measured.
    pub(crate) fn height(&self, style: &Style, now: Duration) -> Option<f32> {
        let to = self.to?;
        let share = progress(style, self.started, now);
        Some(self.from + (to - self.from) * share)
    }
}

/// How far a transition of `style` that started at `started` has gone at `now`, eased: 1 once its
/// duration has passed.
fn progress(style: &Style, started: Duration, now: Duration) -> f32 {
    let duration = style.transition.duration;
    if duration <= 0.0 {
        return 1.0;
    }
    let time = now.saturating_sub(started).as_secs_f32() / duration;
    style.transition.easing.at(time)
}

/// Whether `style` changes over time.
fn moves(style: &Style) -> bool {
    style.transition.duration > 0.0
}

impl Ui {
    /// Gives the interface the time that passed since its last frame, by which its transitions go
    /// on, tooltips wait, submenus open as the pointer rests, and bars of progress without a share
    /// move: a program calls it once a frame, before [`Ui::frame`], with the frame's real time, as
    /// `slime-app` does for a presentation's interface. An interface that is never given time
    /// shows each transition's start until it is, and no tooltip.
    pub fn advance(&mut self, elapsed: Duration) {
        self.now += elapsed;
    }

    /// Shows `style`, the new style of `id`: at once, or from what `id` shows now, over its
    /// transition. An element styled for the first time shows its style at once.
    pub(crate) fn show_style(&mut self, id: ElementId, style: Arc<Style>, first: bool) {
        let (now, reduced) = (self.now, self.preferences.reduced_motion);
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let from = Shown::of(&node.style);
        if first || reduced || !moves(&style) || from == Shown::of(&style) {
            node.style = style;
            node.motion = None;
            return;
        }
        let motion = Motion {
            target: style,
            from,
            started: now,
        };
        let shown = self.shown.get(&motion, 0.0);
        let node = self.node_mut(id).expect("styled above");
        node.style = shown;
        node.motion = Some(Box::new(motion));
        if !self.moving.contains(&id) {
            self.moving.push(id);
        }
    }

    /// Starts `id`, a section, growing to its open height or shrinking to its folded one, over its
    /// transition, from the height that it was laid out at; nothing where its style does not move
    /// or it was never laid out.
    pub(crate) fn reveal(&mut self, id: ElementId, opening: bool) {
        let (now, reduced) = (self.now, self.preferences.reduced_motion);
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let target = node.motion.as_ref().map_or_else(
            || Arc::clone(&node.style),
            |motion| Arc::clone(&motion.target),
        );
        let from = node.layout.size.height;
        if reduced
            || !moves(&target)
            || !matches!(node.element.kind, Kind::Section { .. })
            || from <= 0.0
        {
            node.reveal = None;
            return;
        }
        // Folded, it keeps its padding, the header in it, and its border.
        let layout = node.layout;
        let folded =
            layout.padding.top + layout.padding.bottom + layout.border.top + layout.border.bottom;
        node.reveal = Some(Reveal {
            from,
            to: (!opening).then_some(folded),
            opening,
            started: now,
        });
        self.mark_layout(id);
        if !self.moving.contains(&id) {
            self.moving.push(id);
        }
    }

    /// Moves the transitions under way on to the interface's clock: their elements drawn again, a
    /// section laid out again at its height, each ended once its duration has passed. Gives whether
    /// any goes on.
    pub(crate) fn step_motions(&mut self) -> bool {
        let now = self.now;
        self.shown.clear();
        for id in std::mem::take(&mut self.moving) {
            let Some(node) = self.node_mut(id) else {
                continue;
            };
            let mut going = false;
            if let Some(motion) = node.motion.take() {
                let share = progress(&motion.target, motion.started, now);
                let node = if share >= 1.0 {
                    node.style = Arc::clone(&motion.target);
                    node
                } else {
                    let shown = self.shown.get(&motion, share);
                    let node = self.node_mut(id).expect("moving above");
                    node.style = shown;
                    node.motion = Some(motion);
                    going = true;
                    node
                };
                node.changed.paint = true;
            }
            let node = self.node_mut(id).expect("moving above");
            if let Some(reveal) = node.reveal {
                let ended =
                    reveal.to.is_some() && progress(&node.style, reveal.started, now) >= 1.0;
                if ended {
                    self.end_reveal(id);
                } else {
                    self.mark_layout(id);
                    going = true;
                }
            }
            if going {
                self.moving.push(id);
            }
        }
        !self.moving.is_empty()
    }

    /// Ends the transition of `id` at once: its style's end shown, a section at its height.
    pub(super) fn finish_motion(&mut self, id: ElementId) {
        if let Some(node) = self.node_mut(id) {
            if let Some(motion) = node.motion.take() {
                node.style = motion.target;
                node.changed.paint = true;
            }
            if node.reveal.is_some() {
                self.end_reveal(id);
            }
        }
    }

    /// Ends the section `id`'s growing or shrinking: laid out at the height of its content again, and
    /// what it holds hidden once it has folded.
    fn end_reveal(&mut self, id: ElementId) {
        let Some(node) = self.node_mut(id) else {
            return;
        };
        let opening = node.reveal.take().is_some_and(|reveal| reveal.opening);
        let children = node.children.clone();
        self.mark_layout(id);
        if !opening {
            for child in children {
                self.mark_style(child);
            }
        }
    }

    /// The height that `id`, a section that grows or shrinks, is laid out at now, in pixels of the
    /// screen; none for another element, or a section whose open height is not measured yet.
    pub(crate) fn reveal_height(&self, id: ElementId) -> Option<f32> {
        let node = self.node(id)?;
        node.reveal?.height(node.target(), self.now)
    }

    /// Measures the open height of the sections of `root` that started to open, as the layout that
    /// just ran laid them out, open and at the height of their content, and marks them laid out
    /// again at the height that they start from. Gives whether any was.
    pub(crate) fn measure_reveals(&mut self, root: ElementId) -> bool {
        let mut measured = Vec::new();
        for &id in &self.moving {
            if self.root_of(id) != Some(root) {
                continue;
            }
            if let Some(node) = self.node(id)
                && let Some(reveal) = node.reveal
                && reveal.opening
                && reveal.to.is_none()
            {
                measured.push((id, node.layout.size.height));
            }
        }
        for &(id, height) in &measured {
            if let Some(reveal) = self.node_mut(id).and_then(|node| node.reveal.as_mut()) {
                reveal.to = Some(height);
            }
            self.mark_layout(id);
        }
        !measured.is_empty()
    }

    /// Whether `id` is a section that shrinks as it folds: what it holds shows until it has.
    pub(crate) fn folding(&self, id: ElementId) -> bool {
        self.node(id)
            .and_then(|node| node.reveal)
            .is_some_and(|reveal| !reveal.opening)
    }
}
