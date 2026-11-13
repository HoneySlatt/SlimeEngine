//! The times that a host gives the interface (`docs/adr/0227-the-time-that-motion-reads.md`): each
//! frame's on the host's clock, each root's clock with its speed, and the host's waker. A transition
//! of a state that answers a person follows the host's clock; any other, and a section's, follows
//! its root's, which a paused game holds.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Wake, Waker};
use std::time::Duration;

use slime_color::LinearColor;
use uwui::display::{DisplayList, Primitive};
use uwui::host::{Event, Input, Person, TargetId};
use uwui::style::{Dimension, FlexDirection, StyleBuilder};
use uwui::text::Fonts;
use uwui::theme::{StyleSheet, Theme};
use uwui::time::{EventTime, FrameShown, FrameSource, Instant, RootClock, RootTime};
use uwui::tree::{
    Along, Element, ElementId, ElementState, Layer, LayoutDirection, Notification,
    NotificationKind, Place, RootSettings, Side, Ui,
};

/// A button that goes from black to white over 200 ms as it is selected, and from black to white
/// as the pointer hovers it.
const BUTTONS: &str = r##"
button {
    select kind=Button
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

button-selected {
    select kind=Button { states Selected; }
    paint background="#ffffff"
}

button-hovered {
    select kind=Button { states Hovered; }
    paint background="#ffffff"
}
"##;

const SECTIONS: &str = r##"
section {
    select kind=Section
    transition duration=(s)0.2 easing=Linear
}
"##;

/// An interface styled by `sheet`, with one root of 400 by 300 logical pixels, and the button or
/// section that `build` adds to it.
fn interface(sheet: &str, element: Element) -> (Ui, ElementId, ElementId) {
    let path = format!("{}/tests/fonts/NotoSans.ttf", env!("CARGO_MANIFEST_DIR"));
    let mut fonts = Fonts::new();
    fonts
        .register(std::fs::read(path).expect("the test font"))
        .expect("a font");
    let mut ui = Ui::new(fonts);
    ui.set_style(
        &[StyleSheet::from_text("clocks.style", sheet)],
        &[Theme::from_text("empty.theme", "")],
    )
    .expect("the style sheet");
    let settings = RootSettings {
        size: [400.0, 300.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s.direction(FlexDirection::Column));
    let mut added = None;
    ui.build(root, 0, |children| {
        added = Some(children.add("it", element, |_| {}));
    });
    ui.frame();
    (ui, root, added.expect("built"))
}

fn button() -> Element {
    Element::button("go").style(|s| sized(s, 100.0, 30.0))
}

fn sized(s: StyleBuilder, width: f32, height: f32) -> StyleBuilder {
    s.width(Dimension::Pixels(width))
        .height(Dimension::Pixels(height))
        .shrink(0.0)
        .fonts(vec!["Noto Sans".to_owned()])
}

/// The red of the first box 100 pixels wide: the button's fill, 0 at black and 1 at white.
fn red(list: &DisplayList) -> f32 {
    list.items()
        .iter()
        .find_map(|item| match &item.primitive {
            Primitive::Box(primitive) if primitive.rect.width == 100.0 => Some(primitive.fill),
            _ => None,
        })
        .map(|fill: LinearColor| fill.r)
        .expect("the button")
}

fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

/// A frame shown at `at` on the host's clock.
fn frame_at(ui: &mut Ui, at: Instant) -> DisplayList {
    ui.set_frame_shown(FrameShown::simulated(at));
    ui.frame().0
}

fn select(ui: &mut Ui, id: ElementId) {
    ui.set_state(
        id,
        ElementState {
            selected: true,
            ..ElementState::default()
        },
    );
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn a_paused_roots_transition_holds_where_it_is_and_asks_for_no_frame() {
    let (mut ui, root, button) = interface(BUTTONS, button());
    let start = Instant::from_nanos(3_600_000_000_000);
    let game = |nanos| RootTime::from_nanos(nanos);
    ui.set_root_clock(
        root,
        RootClock {
            time: game(0),
            speed: 1.0,
        },
    );
    frame_at(&mut ui, start);
    select(&mut ui, button);
    assert!(close(red(&frame_at(&mut ui, start)), 0.0));
    // Half of the game's 200 ms: half the way, whatever the host's clock says.
    ui.set_root_clock(
        root,
        RootClock {
            time: game(100_000_000),
            speed: 1.0,
        },
    );
    assert!(close(red(&frame_at(&mut ui, start + ms(16))), 0.5));
    // Paused: the host's time goes on, the game's holds, and so does the transition.
    ui.set_root_clock(
        root,
        RootClock {
            time: game(100_000_000),
            speed: 0.0,
        },
    );
    let list = frame_at(&mut ui, start + ms(500));
    assert!(close(red(&list), 0.5));
    assert!(!ui.animating(), "a paused transition moves nothing");
    assert!(!ui.needs_frame());
    ui.set_frame_shown(FrameShown::simulated(start + ms(516)));
    assert_eq!(ui.frame().1.painted, 0, "nor is drawn again");
    // Resumed, it ends on the game's time.
    ui.set_root_clock(
        root,
        RootClock {
            time: game(200_000_000),
            speed: 1.0,
        },
    );
    assert!(ui.animating());
    assert!(close(red(&frame_at(&mut ui, start + ms(516))), 1.0));
}

#[test]
fn a_hover_follows_the_hosts_clock_while_its_root_is_paused() {
    let (mut ui, root, _) = interface(BUTTONS, button());
    let start = Instant::from_nanos(1_000_000_000);
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::ZERO,
            speed: 0.0,
        },
    );
    frame_at(&mut ui, start);
    ui.pointer_moved(50.0, 15.0);
    assert!(close(red(&frame_at(&mut ui, start)), 0.0));
    assert!(ui.animating(), "the hover moves though the game is paused");
    assert!(close(red(&frame_at(&mut ui, start + ms(100))), 0.5));
    assert!(close(red(&frame_at(&mut ui, start + ms(200))), 1.0));
}

/// A button that a selection turns white over 200 ms, and that a hover does not change.
const SELECTED_ONLY: &str = r##"
button {
    select kind=Button
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

button-selected {
    select kind=Button { states Selected; }
    paint background="#ffffff"
}
"##;

#[test]
fn a_hover_that_changes_no_style_leaves_a_later_selection_on_its_roots_clock() {
    let (mut ui, root, button) = interface(SELECTED_ONLY, button());
    let start = Instant::from_nanos(1_000_000_000);
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::ZERO,
            speed: 0.0,
        },
    );
    frame_at(&mut ui, start);
    // Hovered: its style stays the same.
    ui.pointer_moved(50.0, 15.0);
    frame_at(&mut ui, start);
    // Selected while hovered, the game paused: the selection's transition holds.
    let mut state = ui.state(button).expect("the button");
    assert!(state.hovered);
    state.selected = true;
    ui.set_state(button, state);
    frame_at(&mut ui, start);
    assert!(close(red(&frame_at(&mut ui, start + ms(100))), 0.0));
    assert!(!ui.animating());
}

#[test]
fn a_slowed_root_moves_its_transitions_by_its_own_time() {
    let (mut ui, root, button) = interface(BUTTONS, button());
    let start = Instant::from_nanos(5_000_000_000);
    let quarter = |nanos| RootClock {
        time: RootTime::from_nanos(nanos),
        speed: 0.25,
    };
    ui.set_root_clock(root, quarter(0));
    select(&mut ui, button);
    frame_at(&mut ui, start);
    // 200 ms of the host's at a quarter of the speed: the game gives 50 ms of its own, a quarter
    // of the way, whatever the host's time.
    ui.set_root_clock(root, quarter(50_000_000));
    assert!(close(red(&frame_at(&mut ui, start + ms(200))), 0.25));
    assert!(ui.animating());
}

#[test]
fn a_roots_clock_that_goes_back_keeps_how_far_its_transitions_had_gone() {
    let (mut ui, root, button) = interface(BUTTONS, button());
    let start = Instant::from_nanos(2_000_000_000);
    // A root given no clock follows the host's.
    frame_at(&mut ui, start);
    select(&mut ui, button);
    frame_at(&mut ui, start);
    assert!(close(red(&frame_at(&mut ui, start + ms(50))), 0.25));
    // The game takes its own clock from zero, far behind the host's time.
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::ZERO,
            speed: 1.0,
        },
    );
    assert!(close(red(&frame_at(&mut ui, start + ms(66))), 0.25));
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::from_nanos(50_000_000),
            speed: 1.0,
        },
    );
    assert!(close(red(&frame_at(&mut ui, start + ms(116))), 0.5));
}

#[test]
fn a_section_follows_its_roots_clock_from_its_first_frame() {
    let section = Element::section("physics", true).style(|s| {
        s.direction(FlexDirection::Column)
            .fonts(vec!["Noto Sans".to_owned()])
    });
    let (mut ui, root, section) = interface(SECTIONS, section);
    ui.build(section, 0, |children| {
        children.add(
            "row",
            Element::container().style(|s| sized(s, 100.0, 100.0)),
            |_| {},
        );
    });
    let start = Instant::from_nanos(1_000_000_000);
    frame_at(&mut ui, start);
    let open = ui.rect(section).expect("laid out").height;
    // Folded long before the next frame, its root following the host's clock: it starts at that
    // frame, its first step shown, not at the fold.
    ui.set_open(section, false);
    frame_at(&mut ui, start + ms(400));
    assert!(ui.animating(), "it starts at its first frame");
    assert!((ui.rect(section).expect("laid out").height - open).abs() < 1.0);
    // The game takes its own clock, paused: it holds at its start, open.
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::ZERO,
            speed: 0.0,
        },
    );
    frame_at(&mut ui, start + ms(800));
    assert!((ui.rect(section).expect("laid out").height - open).abs() < 1.0);
    assert!(!ui.animating());
    // Halfway on the game's clock.
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::from_nanos(100_000_000),
            speed: 1.0,
        },
    );
    frame_at(&mut ui, start + ms(816));
    let half = ui.rect(section).expect("laid out").height;
    assert!((half - (open - 50.0)).abs() < 1.0, "{half} of {open}");
}

#[test]
fn the_waiting_bar_keeps_its_place_after_a_month_of_the_hosts_clock() {
    let bar = Element::progress(None).style(|s| sized(s, 200.0, 8.0));
    let (mut ui, _, bar) = interface("", bar);
    // A quarter of its turn of 1.5 s, after 30 days: a third of the track crossing it, from 2.5 %
    // of its width. In `f32` seconds from the clock's zero, the time would step by 0.25 s.
    let month = 1_728_000 * 1_500_000_000;
    let list = frame_at(&mut ui, Instant::from_nanos(month + 375_000_000));
    let track = ui.rect(bar).expect("laid out");
    let moving = list
        .items()
        .iter()
        .find_map(|item| match &item.primitive {
            Primitive::Box(primitive) if (primitive.rect.width - 60.0).abs() < 0.5 => {
                Some(primitive.rect)
            }
            _ => None,
        })
        .expect("the bar's third");
    assert!(
        (moving.x - (track.x + 5.0)).abs() < 0.5,
        "{moving:?} in {track:?}"
    );
}

#[test]
fn a_frame_tells_its_time_and_the_interfaces_time_never_goes_back() {
    let (mut ui, _, _) = interface(BUTTONS, button());
    let time = FrameShown {
        at: Instant::from_nanos(7_000_000_000),
        source: FrameSource::Started,
    };
    ui.set_frame_shown(time);
    assert_eq!(ui.frame().1.shown, time);
    // A notification waits from the frame's time; a frame given an earlier time moves nothing
    // back, so that it ends when it was due.
    ui.notify(Notification {
        kind: NotificationKind::Information,
        text: "Saved".to_owned(),
        ..Notification::default()
    });
    ui.frame();
    let due = Some(time.at + Duration::from_secs(5));
    assert_eq!(ui.next_frame_due(), due);
    ui.set_frame_shown(FrameShown::simulated(Instant::from_nanos(1)));
    assert_eq!(ui.frame().1.shown.at, Instant::from_nanos(1));
    assert_eq!(ui.next_frame_due(), due);
}

#[test]
fn a_speed_below_zero_or_not_a_number_is_refused() {
    for speed in [-1.0, f64::NAN, f64::INFINITY] {
        let refused = std::panic::catch_unwind(|| {
            let (mut ui, root, _) = interface(BUTTONS, button());
            ui.set_root_clock(
                root,
                RootClock {
                    time: RootTime::ZERO,
                    speed,
                },
            );
        });
        assert!(refused.is_err(), "{speed} refused");
    }
}

#[test]
fn a_roots_first_clock_ahead_of_the_hosts_time_keeps_its_transitions_places() {
    let (mut ui, root, button) = interface(BUTTONS, button());
    // The host's clock is the machine's uptime, 600 s; the root follows it.
    let start = Instant::from_nanos(600_000_000_000);
    frame_at(&mut ui, start);
    select(&mut ui, button);
    frame_at(&mut ui, start);
    assert!(close(red(&frame_at(&mut ui, start + ms(50))), 0.25));
    // A save's game time of three hours, far ahead of the uptime.
    let three_hours = 3 * 3600 * 1_000_000_000;
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::from_nanos(three_hours),
            speed: 1.0,
        },
    );
    assert!(close(red(&frame_at(&mut ui, start + ms(66))), 0.25));
    // Then the game's time moves it on, a later time as a late frame does.
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::from_nanos(three_hours + 50_000_000),
            speed: 1.0,
        },
    );
    assert!(close(red(&frame_at(&mut ui, start + ms(116))), 0.5));
}

#[test]
fn a_paused_motion_lays_out_and_draws_nothing_again() {
    let section = Element::section("physics", true).style(|s| {
        s.direction(FlexDirection::Column)
            .fonts(vec!["Noto Sans".to_owned()])
    });
    let (mut ui, root, section) = interface(SECTIONS, section);
    ui.build(section, 0, |children| {
        children.add(
            "row",
            Element::container().style(|s| sized(s, 100.0, 100.0)),
            |_| {},
        );
    });
    let start = Instant::from_nanos(1_000_000_000);
    frame_at(&mut ui, start);
    let game = |nanos, speed| RootClock {
        time: RootTime::from_nanos(nanos),
        speed,
    };
    ui.set_root_clock(root, game(0, 1.0));
    ui.set_open(section, false);
    frame_at(&mut ui, start + ms(16));
    ui.set_root_clock(root, game(50_000_000, 1.0));
    frame_at(&mut ui, start + ms(66));
    // Paused halfway: the frames that the game still draws lay out and draw nothing of it.
    ui.set_root_clock(root, game(50_000_000, 0.0));
    ui.set_frame_shown(FrameShown::simulated(start + ms(82)));
    ui.frame();
    ui.set_frame_shown(FrameShown::simulated(start + ms(98)));
    let (_, stats) = ui.frame();
    assert_eq!((stats.layouts, stats.painted), (0, 0), "{stats:?}");
    assert!(ui.state(section).is_some());
}

#[test]
fn a_layer_follows_the_clock_of_the_root_that_it_opens_over() {
    let (mut ui, root, button) = interface(BUTTONS, button());
    let start = Instant::from_nanos(1_000_000_000);
    ui.set_root_clock(
        root,
        RootClock {
            time: RootTime::ZERO,
            speed: 0.0,
        },
    );
    frame_at(&mut ui, start);
    // A menu of the paused game's interface, beside the button.
    let layer = ui.open_layer(
        "menu",
        Layer {
            place: Place::Beside {
                element: button,
                side: Side::Below,
                along: Along::Start,
            },
            modal: false,
            dismiss: true,
        },
        |s| s.direction(FlexDirection::Column),
    );
    let mut item = None;
    ui.build(layer, 0, |children| {
        item = Some(children.add("item", button_element(), |_| {}));
    });
    frame_at(&mut ui, start);
    let item = item.expect("built");
    select(&mut ui, item);
    frame_at(&mut ui, start);
    frame_at(&mut ui, start + ms(100));
    assert!(!ui.animating(), "the menu's transition holds with the game");
    // A clock given to the layer itself is ignored: it follows its opener.
    ui.set_root_clock(
        layer,
        RootClock {
            time: RootTime::ZERO,
            speed: 1.0,
        },
    );
    frame_at(&mut ui, start + ms(200));
    assert!(!ui.animating());
}

/// A second button, for a layer.
fn button_element() -> Element {
    button()
}

#[test]
fn a_press_and_a_visible_focus_follow_the_hosts_clock_while_the_root_is_paused() {
    for state in ["pressed", "focus"] {
        let (mut ui, root, button) = interface(PERSON, button());
        let start = Instant::from_nanos(1_000_000_000);
        ui.set_root_clock(
            root,
            RootClock {
                time: RootTime::ZERO,
                speed: 0.0,
            },
        );
        frame_at(&mut ui, start);
        let mut held = ui.state(button).expect("the button");
        if state == "pressed" {
            held.pressed = true;
            ui.set_state(button, held);
        } else {
            ui.set_focus(Some(button));
        }
        frame_at(&mut ui, start);
        assert!(ui.animating(), "{state} moves though the game is paused");
        assert!(
            close(red(&frame_at(&mut ui, start + ms(100))), 0.5),
            "{state}"
        );
    }
}

/// A button that a press or a visible focus turns white over 200 ms.
const PERSON: &str = r##"
button {
    select kind=Button
    paint background="#000000"
    transition duration=(s)0.2 easing=Linear
}

button-pressed {
    select kind=Button { states Pressed; }
    paint background="#ffffff"
}

button-focus {
    select kind=Button { states FocusVisible; }
    paint background="#ffffff"
}
"##;

#[test]
fn an_event_earlier_than_the_interfaces_time_moves_nothing_back() {
    let (mut ui, _, _) = interface(BUTTONS, button());
    let at = Instant::from_nanos(9_000_000_000);
    ui.set_frame_shown(FrameShown::simulated(at));
    let early = EventTime::received(Instant::from_nanos(1));
    let _ = ui.event(Event::new(TargetId::MAIN, early, Input::PointerLeft));
    ui.notify(Notification {
        kind: NotificationKind::Information,
        text: "Saved".to_owned(),
        ..Notification::default()
    });
    ui.frame();
    assert_eq!(ui.next_frame_due(), Some(at + Duration::from_secs(5)));
}

/// Counts its wakes.
struct Counter(AtomicUsize);

impl Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn the_hosts_waker_wakes_the_host_from_any_thread() {
    let (mut ui, _, _) = interface(BUTTONS, button());
    // Before the host gives its own, the waker wakes nothing.
    ui.waker().wake_by_ref();
    let counter = Arc::new(Counter(AtomicUsize::new(0)));
    ui.set_waker(Waker::from(Arc::clone(&counter)));
    let waker = ui.waker().clone();
    std::thread::spawn(move || waker.wake())
        .join()
        .expect("the thread woke the host");
    assert_eq!(counter.0.load(Ordering::SeqCst), 1);
}
