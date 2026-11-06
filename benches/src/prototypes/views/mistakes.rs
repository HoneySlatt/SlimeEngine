//! Views' mistakes that the compiler lets through, each a small case
//! (`docs/ideal-code/README.md` in uwui's repository, "Mistakes that the compiler lets through"):
//! a `notify` forgotten; a view within a view changed without its own `notify`; and a view updated
//! while it is updated, which panics, as GPUI's entities do.

use std::ops::Range;

use super::prelude::*;

/// A row's height, in logical pixels.
const ROW: f32 = 32.0;

/// A list whose selection moves by the arrows, whose `select` forgets `notify`.
#[derive(Clone, Default)]
pub struct Forgotten {
    selected: usize,
}

impl Forgotten {
    fn select(&mut self, place: usize, _: &mut Cx<Self>) {
        // The mistake: the selection changes, and the view does not say so.
        self.selected = place;
    }
}

impl Render for Forgotten {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        let rows = cx.processor(|_, rows: Range<usize>, _| {
            rows.map(|row| label(format!("Row {row}")).key(row).class("row"))
                .collect()
        });
        list(1_000, ROW, rows)
            .class("table")
            .selection(self.selected)
            .on_step(cx.listener(|this, &place, cx| this.select(place, cx)))
    }
}

/// A list whose row selected plays by Enter, shown in a bar, a view of its own, which the list
/// changes without its `notify`.
#[derive(Clone)]
pub struct Played {
    selected: usize,
    bar: View<Bar>,
}

#[derive(Clone, Default)]
pub struct Bar {
    playing: Option<usize>,
}

impl Played {
    pub fn new(cx: &mut Cx<Self>) -> Self {
        Self {
            selected: 0,
            bar: cx.new(|_| Bar::default()),
        }
    }

    fn play(&mut self, cx: &mut Cx<Self>) {
        let row = self.selected;
        // The mistake: the bar changes, and does not say so.
        self.bar.update(cx, |bar, _| bar.playing = Some(row));
    }
}

impl Render for Played {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        let rows = cx.processor(|_, rows: Range<usize>, _| {
            rows.map(|row| label(format!("Row {row}")).key(row).class("row"))
                .collect()
        });
        let table = list(1_000, ROW, rows)
            .class("table")
            .selection(self.selected)
            .on_step(cx.listener(|this, &place, cx| {
                this.selected = place;
                cx.notify();
            }))
            .on_accept(cx.listener(|this, _, cx| this.play(cx)));
        column((table, self.bar.clone()))
    }
}

impl Render for Bar {
    fn render(&mut self, _: &mut Cx<Self>) -> impl IntoElement {
        let playing = self.playing.map(|row| format!("Row {row}"));
        row(label(playing.unwrap_or_default()))
    }
}

/// A button in a view within a view, which tells the view around it, which updates the view
/// within again: the view within is being updated already.
#[cfg(test)]
#[derive(Clone)]
pub struct Twice {
    presses: usize,
    inner: View<Inner>,
}

#[cfg(test)]
#[derive(Clone)]
pub struct Inner {
    outer: Option<View<Twice>>,
    presses: usize,
}

#[cfg(test)]
impl Twice {
    pub fn new(cx: &mut Cx<Self>) -> Self {
        Self {
            presses: 0,
            inner: cx.new(|_| Inner {
                outer: None,
                presses: 0,
            }),
        }
    }

    /// Counts a press, and tells the view within, which counts it too.
    fn pressed(&mut self, cx: &mut Cx<Self>) {
        self.presses += 1;
        let presses = self.presses;
        self.inner.update(cx, |inner, cx| {
            inner.presses = presses;
            cx.notify();
        });
        cx.notify();
    }
}

#[cfg(test)]
impl Render for Twice {
    fn render(&mut self, _: &mut Cx<Self>) -> impl IntoElement {
        column((
            label(format!("{} presses", self.presses)),
            self.inner.clone(),
        ))
    }
}

#[cfg(test)]
impl Render for Inner {
    fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
        button(format!("Pressed {}", self.presses))
            .class("primary")
            .on_press(cx.listener(|this, _, cx| {
                if let Some(outer) = this.outer.clone() {
                    outer.update(cx, |outer, cx| outer.pressed(cx));
                }
            }))
    }
}

/// The handle of the view around, given to the view within once both exist.
#[cfg(test)]
pub fn twice(cx: &mut App) -> View<Twice> {
    let twice = cx.new(Twice::new);
    let inner = twice.update(cx, |twice, _| twice.inner.clone());
    let outer = twice.clone();
    inner.update(cx, |inner, _| inner.outer = Some(outer));
    twice
}
