//! With the feature `hot`, writes the views that bring the bench's crate to the size of Slatty's
//! interface, for the reload of a function's body timed under dx (ADR-0212); without it, nothing.

use std::fmt::Write as _;

/// Screens of the library's shape, each some 60 lines once formatted: with the bench's own, the
/// crate holds as many lines of code as Slatty's interface (uwui's `docs/measurements.md`).
const SCREENS: usize = 146;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=SLIME_HOT_SCREENS");
    let screens = std::env::var("SLIME_HOT_SCREENS")
        .ok()
        .map_or(SCREENS, |screens| {
            screens.parse().expect("SLIME_HOT_SCREENS is a count")
        });
    let out = std::env::var("OUT_DIR").expect("cargo gives OUT_DIR");
    let mut code = String::new();
    if std::env::var_os("CARGO_FEATURE_HOT").is_some() {
        for screen in 0..screens {
            write!(code, "{}", SCREEN.replace("{n}", &screen.to_string()))
                .expect("a string takes what is written");
        }
        let names: Vec<String> = (0..screens)
            .map(|screen| format!("screen_{screen}"))
            .collect();
        write!(
            code,
            "/// Every screen, so that each is compiled.\npub(crate) const SCREENS: [fn(Vec<Game>) -> El; {screens}] = [{}];\n",
            names.join(", ")
        )
        .expect("a string takes what is written");
    }
    std::fs::write(format!("{out}/sized.rs"), code).expect("OUT_DIR takes the views");
}

const SCREEN: &str = r#"
/// A screen of the library's shape, the number {n}.
pub(crate) fn screen_{n}(games: Vec<Game>) -> El {
    let games = Stored::new(games);
    let filter = RwSignal::kept("filter", String::new);
    // The games that the filter keeps, by their places in `games`.
    let shown = Memo::new(move || games.with(|games| matching_{n}(games, &filter.get())));
    let selected = RwSignal::kept("selected", || 0);
    let page = RwSignal::kept("page", || None);
    let dark = RwSignal::kept("dark", || true);
    let open = move |place: usize| page.set(shown.with(|shown| shown.get(place).copied()));
    stack((
        column((
            search_field(filter)
                .placeholder("Search the screen {n}")
                .on_change(move |_| selected.set(0)),
            grid(
                move || shown.get(),
                |&game| game,
                [150.0, 200.0],
                move |game| cell_{n}(games, game, page),
            )
            .class("covers-{n}")
            .selection(move || selected.get())
            .on_step(move |place| selected.set(place))
            .on_accept(move || open(selected.get())),
        )),
        dyn_view(move || page.get().map(|game| page_{n}(games, game, page))),
    ))
    .theme(move || if dark.get() { "dark" } else { "light" })
    .on_cancel(move || page.set(None))
    .on_command("toggle-theme-{n}", move || dark.update(|dark| *dark = !*dark))
}

fn cell_{n}(games: Stored<Vec<Game>>, game: usize, page: RwSignal<Option<usize>>) -> El {
    games.with(|games| {
        pressable((image(games[game].cover), label(games[game].title.clone())))
            .class("cell-{n}")
            .on_press(move || page.set(Some(game)))
    })
}

fn page_{n}(games: Stored<Vec<Game>>, game: usize, page: RwSignal<Option<usize>>) -> El {
    games.with(|games| {
        column((
            button("Back to the screen {n}").on_press(move || page.set(None)),
            image(games[game].art).class("art-{n}"),
            label(games[game].title.clone()).class("title-{n}"),
            label(format!("{} games", games.len())).class("count-{n}"),
        ))
        .class("page-{n}")
    })
}

/// The games whose titles hold `filter`, whatever their case.
fn matching_{n}(games: &[Game], filter: &str) -> Vec<usize> {
    let filter = filter.to_lowercase();
    (0..games.len())
        .filter(|&game| games[game].title.to_lowercase().contains(&filter))
        .collect()
}
"#;
