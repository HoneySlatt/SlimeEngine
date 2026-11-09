//! Slatty's library written as layouts (M6d.1d2a): the library of `library.rs`, its state, its
//! handlers and its games in the view's code as there, its elements in the bench's file
//! (`layouts/library.kdl`), bound by their names.
//!
//! The view's function runs once. The library's top layout is a dynamic view that reads the
//! layouts' signal: when the file changes ([`Signals::relayout`](super::Signals::relayout)), it is
//! made again with its bindings, and so are the covers of the cells that show and the game's page,
//! made from the layouts that it read. The elements that keep their names keep their state: the
//! grid its scroll and its selection, the field its text.
//!
//! The back button names the command that closes the page, `page.close`, which the root's handler
//! runs, as a shortcut that names it would.
//!
//! A limit: the layouts' signal is not kept, so a reload of the view's code (`Signals::reload`)
//! shows the bench's own file again, the last one given lost until the next save.

use super::El;
use super::layout::{Layouts, layout};
use super::library::{CELL, Game, matching};
use super::prelude::*;

/// The library of `games`, its elements made from `layouts`; its handle the layouts' signal.
pub fn library(games: Vec<Game>, layouts: Layouts) -> (RwSignal<Layouts>, impl View) {
    let layouts = RwSignal::new(layouts);
    let games = Stored::new(games);
    let filter = RwSignal::kept("filter", String::new);
    // The games that the filter keeps, by their places in `games`.
    let shown = Memo::new(move || games.with(|games| matching(games, &filter.get())));
    // The place in `shown` of the game selected.
    let selected = RwSignal::kept("selected", || 0);
    // The game whose page is open.
    let page = RwSignal::kept("page", || None);
    let dark = RwSignal::kept("dark", || true);
    let open = move |place: usize| page.set(shown.with(|shown| shown.get(place).copied()));
    let view = dyn_view(move || {
        let laid = layouts.get();
        let covers = laid.clone();
        let pages = laid.clone();
        let library = layout(&laid, "library")
            .bind("search", |el| {
                el.value(filter).on_change(move |_| selected.set(0))
            })
            .bind("covers", move |el| {
                el.cells(
                    move || shown.get(),
                    |&game| game,
                    CELL,
                    move |game| cover(&covers, games, game, page),
                )
                .selection(move || selected.get())
                .on_step(move |place| selected.set(place))
                .on_accept(move || open(selected.get()))
            })
            .slot("page", move || {
                page.get().map(|game| game_page(&pages, games, game))
            })
            .el()
            .theme(move || if dark.get() { "dark" } else { "light" })
            .on_cancel(move || page.set(None))
            .on_command("page.close", move || page.set(None))
            .on_command("toggle-theme", move || dark.update(|dark| *dark = !*dark));
        Some(library)
    });
    (layouts, view)
}

/// The cell of the game at `game` in `games`, from the layout `cover`: pressed, it opens the game's
/// page.
fn cover(
    layouts: &Layouts,
    games: Stored<Vec<Game>>,
    game: usize,
    page: RwSignal<Option<usize>>,
) -> El {
    games.with(|games| {
        layout(layouts, "cover")
            .bind("cover", |el| el.on_press(move || page.set(Some(game))))
            .bind("art", |el| el.image(games[game].cover))
            .bind("title", |el| el.text(games[game].title.clone()))
            .el()
    })
}

/// The page of the game at `game` in `games`, from the layout `game-page`.
fn game_page(layouts: &Layouts, games: Stored<Vec<Game>>, game: usize) -> El {
    games.with(|games| {
        layout(layouts, "game-page")
            .bind("key-art", |el| el.image(games[game].art))
            .bind("title", |el| el.text(games[game].title.clone()))
            .el()
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use uwui::prelude::{Action, ActionKind};

    use super::super::kept::{labels, opened, settled};
    use super::super::layout::{layout, read};
    use super::super::prelude::*;
    use crate::prototypes::host::{Find, Input, Model};
    use crate::prototypes::layout_reload::layouts_with_subtitle;
    use crate::prototypes::reload::{kept, played};
    use crate::prototypes::signals::LAID_LIBRARY;

    #[test]
    fn the_back_button_runs_the_command_that_its_layout_names() {
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        assert!(kept(&host).page, "played to a game's page");
        host.frame(&[Input::PointerAt(Find::Child("page", 0)), Input::Click]);
        settled(&mut host);
        assert!(!kept(&host).page, "the back button closed the page");
    }

    #[test]
    fn the_command_that_a_shortcut_names_runs_the_same_handler() {
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        let command = Action {
            element: None,
            kind: ActionKind::Command("page.close".to_owned()),
        };
        host.program.act(&mut host.ui, command);
        settled(&mut host);
        assert!(!kept(&host).page, "the command closed the page");
    }

    #[test]
    fn after_a_relayout_the_page_closes_and_a_cover_opens_it_again() {
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        host.program
            .relayout(&layouts_with_subtitle())
            .expect("it reads");
        settled(&mut host);
        host.frame(&[Input::PointerAt(Find::Child("page", 0)), Input::Click]);
        settled(&mut host);
        assert!(!kept(&host).page, "the back button of the page made again");
        // The cells, made again by the grid's new rows, take presses.
        host.frame(&[Input::PointerAt(Find::Class("cover")), Input::Click]);
        settled(&mut host);
        assert!(kept(&host).page, "a cover opened its page");
    }

    #[test]
    fn the_laid_library_renders_as_a_new_interface_would() {
        let wrong = crate::prototypes::tests::wrong(&LAID_LIBRARY, false);
        assert!(wrong.iter().all(|&wrong| wrong == 0), "{wrong:?}");
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        host.program
            .relayout(&layouts_with_subtitle())
            .expect("it reads");
        settled(&mut host);
        let (wrong, all) = host.wrong_items().expect("it compares");
        assert_eq!(wrong, 0, "{wrong} of {all} items differ after a relayout");
    }

    #[test]
    fn a_frame_at_rest_after_a_relayout_runs_and_builds_nothing() {
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        host.program
            .relayout(&layouts_with_subtitle())
            .expect("it reads");
        settled(&mut host);
        let still = host.frame(&[]);
        // The grid's rows before ended with the view made again: no builder of theirs runs.
        assert_eq!((still.runs, still.stats.built), (0, 0));
    }

    #[test]
    fn the_theme_changes_after_a_relayout() {
        let mut host = played(&LAID_LIBRARY);
        settled(&mut host);
        host.program
            .relayout(&layouts_with_subtitle())
            .expect("it reads");
        settled(&mut host);
        let before = host.ui.theme().map(str::to_owned);
        host.frame(&[Input::Control('t')]);
        settled(&mut host);
        assert_ne!(host.ui.theme().map(str::to_owned), before);
    }

    const FIRST: &str = "layout shown {\n    column {\n        label a\n    }\n}\n";
    const SECOND: &str =
        "layout shown {\n    column {\n        label before\n        label a\n    }\n}\n";

    #[test]
    fn a_relayout_runs_the_layouts_view_and_its_bindings_but_not_the_views_function() {
        // Calls of the view's function, of the layout's dynamic view, and of the binding.
        let counts = Rc::new(Cell::new([0; 3]));
        let count = move |counts: &Cell<[usize; 3]>, at: usize| {
            let mut held = counts.get();
            held[at] += 1;
            counts.set(held);
        };
        let counted = Rc::clone(&counts);
        let mut host = opened(move || {
            count(&counted, 0);
            let layouts = RwSignal::new(read(FIRST).expect("it reads"));
            let counted = Rc::clone(&counted);
            let view = dyn_view(move || {
                count(&counted, 1);
                let laid = layouts.get();
                let bound = layout(&laid, "shown").bind("a", |el| {
                    count(&counted, 2);
                    el.text("bound")
                });
                Some(bound.el())
            });
            (layouts, view)
        });
        assert_eq!(counts.get(), [1, 1, 1]);
        for text in [SECOND, FIRST] {
            host.program.relayout(text).expect("it reads");
            settled(&mut host);
        }
        assert_eq!(counts.get(), [1, 3, 3]);
        assert_eq!(labels(&host), ["bound"]);
        host.program.relayout(SECOND).expect("it reads");
        let frame = host.frame(&[]);
        assert_eq!(labels(&host), ["", "bound"]);
        assert_eq!(frame.runs, 1, "the layout's dynamic view alone ran again");
        assert!(
            host.program.notes().is_empty(),
            "{:?}",
            host.program.notes()
        );
    }
}
