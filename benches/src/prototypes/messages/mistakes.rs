//! Messages' mistakes that the compiler lets through, each a small case
//! (`docs/ideal-code/README.md` in uwui's repository, "Mistakes that the compiler lets through"):
//! a `lazy` part whose value leaves out what it reads, and a subscription to frames not given up.

use super::prelude::*;

/// A row's height, in logical pixels.
const ROW: f32 = 32.0;

#[derive(Clone, Debug)]
pub enum Message {
    Select(usize),
    Play,
    Frame,
}

/// A list whose row selected plays by Enter; its `lazy` part's value leaves out the row playing.
#[derive(Clone, Default)]
pub struct Playing {
    selected: usize,
    playing: Option<usize>,
}

impl Playing {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::Select(place) => self.selected = place,
            Message::Play => self.playing = Some(self.selected),
            Message::Frame => {}
        }
    }

    pub fn view(&self) -> El<'_, Message> {
        // The mistake: the rows read `self.playing`, which the value leaves out.
        lazy(self.selected, || {
            list(1_000, ROW, |row| {
                label(format!("Row {row}"))
                    .key(row)
                    .class("row")
                    .class_if("playing", self.playing == Some(row))
                    .into()
            })
            .class("table")
            .selection(self.selected)
            .on_step(Message::Select)
            .on_accept(Message::Play)
        })
    }
}

/// A list whose selection moves, which asks for a message at each frame though nothing moves.
#[derive(Clone, Default)]
pub struct Forever {
    selected: usize,
}

impl Forever {
    pub fn update(&mut self, message: Message) {
        if let Message::Select(place) = message {
            self.selected = place;
        }
    }

    pub fn view(&self) -> El<'_, Message> {
        list(1_000, ROW, |row| {
            label(format!("Row {row}")).key(row).class("row").into()
        })
        .class("table")
        .selection(self.selected)
        .on_step(Message::Select)
        .into()
    }

    /// The mistake: frames asked for whatever the state, where nothing moves.
    pub fn subscription(&self) -> Subscription<Message> {
        frames().map(|_| Message::Frame)
    }
}

/// A cover's cell, in logical pixels.
#[cfg(test)]
const CELL: [f32; 2] = [150.0, 200.0];

/// A grid of six covers, each keyed by its game, or, the mistake, left without a key: its state,
/// the focus, passes to another game when one before it is removed.
#[cfg(test)]
#[derive(Clone)]
pub struct Covers {
    shown: Vec<usize>,
    keyed: bool,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub enum Removal {
    RemoveFirst,
}

#[cfg(test)]
impl Covers {
    pub fn new(keyed: bool) -> Self {
        Self {
            shown: (0..6).collect(),
            keyed,
        }
    }

    pub fn update(&mut self, _: Removal) {
        self.shown.remove(0);
    }

    pub fn view(&self) -> El<'_, Removal> {
        grid(self.shown.len(), CELL, |place| {
            let game = self.shown[place];
            let cover = pressable(label(format!("Game {game}"))).class("cover");
            if self.keyed { cover.key(game) } else { cover }.into()
        })
        .class("table")
        .into()
    }
}
