//! Changes from another thread with signals: Slatty's downloads, a list of games whose progress
//! the application's core sends from its own threads, one game at a time or in batches, by the
//! game's place. The core holds a [`Sender`], never a signal; the view's task receives each
//! update and sets the game's signal, so that only its row's label runs again, if it shows. The
//! receiver is the application's, made before the view: a reload's new view reads it again, and
//! the updates counted are kept by their name.
//!
//! The model's concepts, past those of the other screens:
//! - `channel`, whose `Sender` any thread holds and whose `Receiver` the interface's thread reads
//! - `spawn`, a task of the view's scope, cancelled with it
//! - `async`, `.await` and `recv`, which ends once every sender is dropped
//! - the rule: a channel that must outlive a reload is made outside the view

use std::rc::Rc;

use super::prelude::*;

/// A row's height, in logical pixels.
const ROW: f32 = 32.0;

/// What the core sends: a game's progress, in percent, or many games' at once.
#[derive(Debug)]
pub enum Progress {
    One(usize, u8),
    Batch(Vec<(usize, u8)>),
}

/// The downloads of `games` games, whose progress comes through `updates`.
pub fn downloads(games: usize, updates: Rc<Receiver<Progress>>) -> impl View {
    let progress: Rc<[RwSignal<u8>]> = (0..games).map(|_| RwSignal::new(0)).collect();
    let received = RwSignal::kept("received", || 0_u64);
    let rows = Rc::clone(&progress);
    spawn(async move {
        while let Some(update) = updates.recv().await {
            match update {
                Progress::One(game, percent) => progress[game].set(percent),
                Progress::Batch(batch) => {
                    for (game, percent) in batch {
                        progress[game].set(percent);
                    }
                }
            }
            received.update(|received| *received += 1);
        }
    });
    column((
        label(move || format!("{} updates", received.get())).class("received"),
        list(games, ROW, move |game| {
            let percent = rows[game];
            label(move || format!("Game {game}: {}%", percent.get())).class("row")
        })
        .class("table"),
    ))
}
