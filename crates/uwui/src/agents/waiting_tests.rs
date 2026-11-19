//! The family's waits, read from their requests and answered ([`wait`], [`met`]): `after` as the
//! elements seen, the requests refused, the page and its handle; and the waits that a change not
//! yet framed defers.

use uwui_agent::interface::{MAX_SEEN, Reply, Request};
use uwui_agent::page::Page;

use super::*;
use crate::host::{TargetId, TargetSize};
use crate::style::{Dimension, StyleBuilder};
use crate::text::Fonts;
use crate::tree::{Element, ElementState, Role, RootSettings};

/// A page as a tool of a long line asks it.
fn page() -> Page {
    Page::new(1, usize::MAX)
}

fn sized(s: StyleBuilder) -> StyleBuilder {
    s.width(Dimension::Pixels(60.0))
        .height(Dimension::Pixels(20.0))
        .shrink(0.0)
}

/// An interface of a root `main` on the main target, 400 × 300: a button for each of `buttons`,
/// and a list to choose from, `pick`; drawn.
fn interface(buttons: &[&str]) -> (Ui, ElementId) {
    let mut ui = Ui::new(Fonts::new());
    ui.set_target_size(
        TargetId::MAIN,
        TargetSize::new([400, 300], 1.0).expect("a size"),
    );
    let root = ui.add_root("main", RootSettings::on(TargetId::MAIN), |s| s);
    build(&mut ui, root, 0, buttons);
    ui.frame(TargetId::MAIN);
    (ui, root)
}

fn build(ui: &mut Ui, root: ElementId, revision: u64, buttons: &[&str]) {
    ui.build(root, revision, |children| {
        for &name in buttons {
            children.add(name, Element::button(name).style(sized), |_| {});
        }
        let pick = Element::choice(["one", "two"], 0).style(sized);
        children.add("pick", pick, |_| {});
    });
}

/// A wait of `role`, present or `absent`, from `after`.
fn asking(role: &str, absent: bool, after: Option<&str>) -> Request {
    Request::Wait {
        target: 0,
        role: role.to_owned(),
        name: String::new(),
        holding: Vec::new(),
        lacking: Vec::new(),
        absent,
        after: after.map(str::to_owned),
        limit: None,
        bytes: None,
    }
}

/// `request`, a wait, with what `edit` changes.
fn edited(request: &Request, edit: impl FnOnce(&mut Asking)) -> Request {
    let Request::Wait {
        target,
        role,
        name,
        holding,
        lacking,
        absent,
        after,
        limit,
        bytes,
    } = request.clone()
    else {
        panic!("no wait");
    };
    let mut asking = Asking {
        target,
        role,
        name,
        holding,
        lacking,
        absent,
        after,
        limit,
        bytes,
    };
    edit(&mut asking);
    let Asking {
        target,
        role,
        name,
        holding,
        lacking,
        absent,
        after,
        limit,
        bytes,
    } = asking;
    Request::Wait {
        target,
        role,
        name,
        holding,
        lacking,
        absent,
        after,
        limit,
        bytes,
    }
}

/// A wait's fields, which a test changes.
struct Asking {
    target: u32,
    role: String,
    name: String,
    holding: Vec<String>,
    lacking: Vec<String>,
    absent: bool,
    after: Option<String>,
    limit: Option<u64>,
    bytes: Option<u64>,
}

/// A wait's reply: its elements' paths, its `after` and its `more`.
type Said = (Vec<String>, String, bool);

/// What `reply`, a wait's, says.
fn waited(reply: Reply) -> Said {
    let Reply::Waited {
        elements,
        after,
        more,
    } = reply
    else {
        panic!("no wait's reply: {reply:?}");
    };
    (
        elements.into_iter().map(|element| element.path).collect(),
        after,
        more,
    )
}

/// The reply of a wait met at once.
fn at_once(answer: Result<Waiting, Fault>) -> Said {
    match answer {
        Ok(Waiting::Reply(reply)) => waited(reply),
        other => panic!("not met at once: {other:?}"),
    }
}

/// The wait that `answer` holds.
fn held(answer: Result<Waiting, Fault>) -> WaitId {
    match answer {
        Ok(Waiting::Held(wait)) => wait,
        other => panic!("not held: {other:?}"),
    }
}

/// A frame of the main target, then a check of its waits: the replies of those met, by the
/// requests that `asked` gives for each wait.
fn frame_and_answer(
    ui: &mut Ui,
    waits: &mut Waits,
    asked: &[(WaitId, &Request)],
) -> Vec<(WaitId, Said)> {
    ui.frame(TargetId::MAIN);
    let check = waits.check(ui, TargetId::MAIN);
    check
        .met
        .iter()
        .map(|met_wait| {
            let (_, request) = asked
                .iter()
                .find(|(wait, _)| *wait == met_wait.wait)
                .expect("a wait asked");
            let reply = met(ui, request, met_wait, page(), || false).expect("a reply");
            (met_wait.wait, waited(reply))
        })
        .collect()
}

fn element(ui: &Ui, path: &str) -> ElementId {
    ui.element_by_path(path).expect("an element there")
}

fn disabled(disabled: bool) -> ElementState {
    ElementState {
        disabled,
        ..ElementState::default()
    }
}

#[test]
fn a_presence_from_its_after_waits_for_an_element_not_seen() {
    let (mut ui, root) = interface(&["open", "save"]);
    let mut waits = Waits::default();
    let first = asking("button", false, None);
    let (given, after, more) = at_once(wait(&mut ui, &mut waits, &first, page(), || false));
    assert_eq!(given, ["main/open", "main/save"]);
    assert!(!more);
    // From that answer: held, nothing new.
    let again = asking("Button", false, Some(&after));
    let wait_id = held(wait(&mut ui, &mut waits, &again, page(), || false));
    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &again)]).is_empty());
    // A button built: given alone, and the handle holds all three.
    build(&mut ui, root, 1, &["open", "save", "close"]);
    let met = frame_and_answer(&mut ui, &mut waits, &[(wait_id, &again)]);
    let [(id, (given, after, false))] = &met[..] else {
        panic!("one wait met: {met:?}");
    };
    assert_eq!(
        (*id, given.as_slice()),
        (wait_id, &["main/close".to_owned()][..])
    );
    let third = asking("button", false, Some(after));
    held(wait(&mut ui, &mut waits, &third, page(), || false));
    waits.clear(Some(&mut ui));
}

#[test]
fn an_element_seen_that_matched_again_while_held_or_one_not_given_meets_a_presence() {
    let (mut ui, _) = interface(&["open", "save"]);
    let mut waits = Waits::default();
    let enabled = edited(&asking("button", false, None), |w| {
        w.lacking = vec!["Disabled".to_owned()];
    });
    let (_, after, _) = at_once(wait(&mut ui, &mut waits, &enabled, page(), || false));
    let from = edited(&enabled, |w| {
        w.after = Some(after.clone());
    });
    let wait_id = held(wait(&mut ui, &mut waits, &from, page(), || false));
    // `save` disabled, then enabled again while the wait is held: seen no longer, it meets it.
    ui.set_state(element(&ui, "main/save"), disabled(true));
    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &from)]).is_empty());
    ui.set_state(element(&ui, "main/save"), disabled(false));
    let met = frame_and_answer(&mut ui, &mut waits, &[(wait_id, &from)]);
    assert_eq!(met.len(), 1, "{met:?}");
    assert_eq!(met[0].1.0, ["main/save"]);
    // The focus, given to `open` and seen, then to `save` between two waits, which was not given:
    // met at once by `save` alone.
    let focused = edited(&asking("", false, None), |w| {
        w.holding = vec!["focused".to_owned()];
    });
    ui.set_focus(Some(element(&ui, "main/open")));
    ui.frame(TargetId::MAIN);
    let (given, after, _) = at_once(wait(&mut ui, &mut waits, &focused, page(), || false));
    assert_eq!(given, ["main/open"]);
    ui.set_focus(Some(element(&ui, "main/save")));
    ui.frame(TargetId::MAIN);
    let from = edited(&focused, |w| {
        w.after = Some(after);
    });
    let (given, _, _) = at_once(wait(&mut ui, &mut waits, &from, page(), || false));
    assert_eq!(given, ["main/save"]);
    // A walk of the target whole, which a wait held unwalked makes, lets an element that stopped
    // matching out of what was seen: enabled again, it meets the wait.
    let (_, after, _) = at_once(wait(&mut ui, &mut waits, &enabled, page(), || false));
    let from = edited(&enabled, |w| {
        w.after = Some(after);
    });
    let wait_id = held(wait(&mut ui, &mut waits, &from, page(), || false));
    ui.set_state(element(&ui, "main/save"), disabled(true));
    let unwalked = asking("Dialog", false, None);
    held(wait(&mut ui, &mut waits, &unwalked, page(), || false));
    ui.frame(TargetId::MAIN);
    let whole = waits.check(&mut ui, TargetId::MAIN);
    assert!(whole.met.is_empty() && whole.visited > 2, "{whole:?}");
    ui.set_state(element(&ui, "main/save"), disabled(false));
    let met = frame_and_answer(&mut ui, &mut waits, &[(wait_id, &from)]);
    assert_eq!(met.len(), 1, "{met:?}");
    assert_eq!(met[0].1.0, ["main/save"]);
}

#[test]
fn a_list_to_choose_from_closed_is_seen_once() {
    let (mut ui, _) = interface(&["open"]);
    let mut waits = Waits::default();
    let pick = element(&ui, "main/pick");
    // Its options open on a press.
    act(&mut ui, "main/pick", &uwui_agent::interface::Action::Press).expect("pressed");
    ui.frame(TargetId::MAIN);
    let opened = asking("ListBox", false, None);
    let (given, after, _) = at_once(wait(&mut ui, &mut waits, &opened, page(), || false));
    assert_eq!(given.len(), 1, "{given:?}");
    // Closed, from what the presence gave: held while open, met once closed.
    let closed = asking("listbox", true, Some(&after));
    let wait_id = held(wait(&mut ui, &mut waits, &closed, page(), || false));
    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &closed)]).is_empty());
    ui.set_focus(Some(pick));
    act(&mut ui, "", &uwui_agent::interface::Action::Cancel).expect("cancelled");
    let met = frame_and_answer(&mut ui, &mut waits, &[(wait_id, &closed)]);
    let [(_, (given, after, false))] = &met[..] else {
        panic!("closed once: {met:?}");
    };
    assert!(given.is_empty());
    let closed_after = after.clone();
    // Seen once: waited again from its answer, it waits for the list to open and close again.
    let again = asking("ListBox", true, Some(after));
    let wait_id = held(wait(&mut ui, &mut waits, &again, page(), || false));
    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &again)]).is_empty());
    act(&mut ui, "main/pick", &uwui_agent::interface::Action::Press).expect("pressed");
    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &again)]).is_empty());
    act(&mut ui, "", &uwui_agent::interface::Action::Cancel).expect("cancelled");
    assert_eq!(
        frame_and_answer(&mut ui, &mut waits, &[(wait_id, &again)]).len(),
        1
    );
    // From an empty handle, a list already open as the wait starts arms it: closed, it meets it.
    act(&mut ui, "main/pick", &uwui_agent::interface::Action::Press).expect("pressed");
    ui.frame(TargetId::MAIN);
    let open_at_start = asking("ListBox", true, Some(&closed_after));
    let wait_id = held(wait(&mut ui, &mut waits, &open_at_start, page(), || false));
    ui.set_focus(Some(pick));
    act(&mut ui, "", &uwui_agent::interface::Action::Cancel).expect("cancelled");
    assert_eq!(
        frame_and_answer(&mut ui, &mut waits, &[(wait_id, &open_at_start)]).len(),
        1
    );
    // Without `after`, an absence reads the interface as it stands: met at once.
    let (given, _, _) = at_once(wait(
        &mut ui,
        &mut waits,
        &asking("ListBox", true, None),
        page(),
        || false,
    ));
    assert!(given.is_empty());
}

#[test]
fn an_absence_from_elements_seen_and_gone_since_is_met_at_once() {
    let (mut ui, root) = interface(&["open", "save"]);
    let mut waits = Waits::default();
    let saves = edited(&asking("button", false, None), |w| {
        w.name = "save".to_owned();
    });
    let (_, after, _) = at_once(wait(&mut ui, &mut waits, &saves, page(), || false));
    build(&mut ui, root, 1, &["open"]);
    ui.frame(TargetId::MAIN);
    let gone = edited(&saves, |w| {
        w.absent = true;
        w.after = Some(after);
    });
    let (given, after, _) = at_once(wait(&mut ui, &mut waits, &gone, page(), || false));
    assert!(given.is_empty());
    // An empty handle is not none: the absence seen, it waits for one to match and go.
    let again = edited(&gone, |w| {
        w.after = Some(after);
    });
    held(wait(&mut ui, &mut waits, &again, page(), || false));
}

#[test]
fn what_no_element_could_match_and_handles_not_of_the_wait_are_refused() {
    let (mut ui, _) = interface(&["open"]);
    let mut waits = Waits::default();
    let bad = |ui: &mut Ui, waits: &mut Waits, request: &Request| match wait(
        ui,
        waits,
        request,
        page(),
        || false,
    ) {
        Err(fault) => fault,
        other => panic!("not refused: {other:?}"),
    };
    for (request, said) in [
        (asking("Buton", false, None), "no role `Buton`"),
        (
            asking("GenericContainer", true, None),
            "never means something",
        ),
        (
            edited(&asking("", false, None), |w| {
                w.holding = vec!["pressd".to_owned()];
            }),
            "no state `pressd`",
        ),
        (
            edited(&asking("", true, None), |w| {
                w.holding = vec!["Disabled".to_owned()];
                w.lacking = vec!["disabled".to_owned()];
            }),
            "both held and lacking",
        ),
    ] {
        let fault = bad(&mut ui, &mut waits, &request);
        assert_eq!(fault.code, Fault::BAD_PARAMS, "{request:?}");
        assert!(fault.message.contains(said), "{}", fault.message);
    }
    let (_, after, _) = at_once(wait(
        &mut ui,
        &mut waits,
        &asking("button", false, None),
        page(),
        || false,
    ));
    // Another condition, but its presence: refused, so that a button's handle meets no dialog's
    // absence.
    let fault = bad(&mut ui, &mut waits, &asking("Dialog", true, Some(&after)));
    assert_eq!(fault.code, Fault::BAD_PARAMS);
    // The same condition absent takes it.
    held(wait(
        &mut ui,
        &mut waits,
        &asking("button", true, Some(&after)),
        page(),
        || false,
    ));
    // A digit changed: its sum no longer holds.
    let (head, numbers) = after.rsplit_once(':').expect("a handle");
    let first = numbers.split('.').next().expect("a number");
    let changed = format!(
        "{head}:{:x}{}",
        u64::from_str_radix(first, 16).expect("hexadecimal") + 1,
        &numbers[first.len()..]
    );
    let fault = bad(
        &mut ui,
        &mut waits,
        &asking("button", false, Some(&changed)),
    );
    assert_eq!(fault.code, Fault::BAD_PARAMS);
    assert!(
        fault.message.contains("not the `after`"),
        "{}",
        fault.message
    );
    waits.clear(Some(&mut ui));
    // Another interface's: stale.
    let (mut other, _) = interface(&["open"]);
    let fault = bad(
        &mut other,
        &mut waits,
        &asking("button", false, Some(&after)),
    );
    assert_eq!(fault.code, Fault::STALE);
}

#[test]
fn a_page_past_gives_more_and_its_handle_counts_in_its_bytes() {
    let names: Vec<String> = (0..40).map(|at| format!("button {at}")).collect();
    let named: Vec<&str> = names.iter().map(String::as_str).collect();
    let (mut ui, _) = interface(&named);
    let mut waits = Waits::default();
    let few = edited(&asking("button", false, None), |w| {
        w.limit = Some(15);
    });
    let (given, after, more) = at_once(wait(&mut ui, &mut waits, &few, page(), || false));
    assert_eq!((given.len(), more), (15, true));
    // The rest at once, and none twice.
    let mut seen = given;
    let mut after = after;
    while seen.len() < 40 {
        let next = edited(&few, |w| {
            w.after = Some(after);
        });
        let (given, handle, _) = at_once(wait(&mut ui, &mut waits, &next, page(), || false));
        assert!(given.iter().all(|path| !seen.contains(path)), "{given:?}");
        seen.extend(given);
        after = handle;
    }
    // A page of bytes, from a handle of 30 elements seen: its elements and its handle within
    // them, the first element always given.
    let thirty = edited(&few, |w| {
        w.limit = Some(30);
    });
    let (_, of_thirty, _) = at_once(wait(&mut ui, &mut waits, &thirty, page(), || false));
    for bytes in [600, 1_500, 3_000] {
        let paged = edited(&few, |w| {
            w.bytes = Some(bytes);
            w.limit = None;
            w.after = Some(of_thirty.clone());
        });
        let Ok(Waiting::Reply(Reply::Waited {
            elements, after, ..
        })) = wait(&mut ui, &mut waits, &paged, page(), || false)
        else {
            panic!("met at once");
        };
        let held = uwui_agent::json_size(&elements) + after.len();
        assert!(
            elements.len() == 1 || held <= bytes as usize,
            "{held} bytes for a page of {bytes}"
        );
    }
}

#[test]
fn a_wait_that_would_see_more_than_its_most_is_refused() {
    let names: Vec<String> = (0..=MAX_SEEN).map(|at| format!("b{at}")).collect();
    let named: Vec<&str> = names.iter().map(String::as_str).collect();
    let (mut ui, _) = interface(&named);
    let mut waits = Waits::default();
    let fault = match wait(
        &mut ui,
        &mut waits,
        &asking("button", false, None),
        page(),
        || false,
    ) {
        Err(fault) => fault,
        other => panic!("not refused: {other:?}"),
    };
    assert_eq!(fault.code, Fault::BAD_PARAMS);
    assert!(fault.message.contains("narrower"), "{}", fault.message);
}

#[test]
fn a_wait_while_its_target_holds_a_change_not_framed_is_walked_after_the_frame() {
    let (mut ui, root) = interface(&["open"]);
    let mut waits = Waits::default();
    // Built, not framed: read after the frame, where its style hides it.
    ui.build(root, 1, |children| {
        children.add("open", Element::button("open").style(sized), |_| {});
        let hidden =
            Element::button("hidden").style(|s| sized(s).display(crate::style::Display::None));
        children.add("hidden", hidden, |_| {});
        children.add(
            "pick",
            Element::choice(["one", "two"], 0).style(sized),
            |_| {},
        );
    });
    let hidden = edited(&asking("button", false, None), |w| {
        w.name = "hidden".to_owned();
    });
    let wait_id = held(wait(&mut ui, &mut waits, &hidden, page(), || false));
    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &hidden)]).is_empty());
    waits.clear(Some(&mut ui));
    // A change not framed: a check reads nothing, and keeps its marks for the one after the frame.
    let enabled = edited(&asking("button", false, None), |w| {
        w.name = "open".to_owned();
        w.lacking = vec!["disabled".to_owned()];
    });
    ui.set_state(element(&ui, "main/open"), disabled(true));
    ui.frame(TargetId::MAIN);
    let wait_id = held(wait(&mut ui, &mut waits, &enabled, page(), || false));
    ui.set_state(element(&ui, "main/open"), disabled(false));
    let early = waits.check(&mut ui, TargetId::MAIN);
    assert_eq!(early, WaitCheck::default(), "nothing read before the frame");
    let met = frame_and_answer(&mut ui, &mut waits, &[(wait_id, &enabled)]);
    assert_eq!(met.len(), 1, "{met:?}");
}

#[test]
fn an_answer_without_waits_refuses_a_wait() {
    let (mut ui, _) = interface(&["open"]);
    let refused = answer(&mut ui, &asking("button", false, None), page(), || false);
    let Err(fault) = refused else {
        panic!("a wait answered: {refused:?}");
    };
    assert_eq!(fault.code, Fault::NO_METHOD);
    assert!(
        fault.message.contains("does not hold waits"),
        "{}",
        fault.message
    );
}

#[test]
fn a_wait_from_after_held_unwalked_keeps_its_elements_seen() {
    let (mut ui, root) = interface(&["open", "save"]);
    let mut waits = Waits::default();
    let buttons = asking("Button", false, None);
    let (_, after, _) = at_once(wait(&mut ui, &mut waits, &buttons, page(), || false));
    // A button hovered, not framed: the target holds a change, and the wait from `after` is held
    // unwalked, then walked after the frame, its elements seen kept.
    let hovered = ElementState {
        hovered: true,
        ..ElementState::default()
    };
    ui.set_state(element(&ui, "main/save"), hovered);
    let from = asking("Button", false, Some(&after));
    let wait_id = held(wait(&mut ui, &mut waits, &from, page(), || false));

    assert!(frame_and_answer(&mut ui, &mut waits, &[(wait_id, &from)]).is_empty());
    build(&mut ui, root, 1, &["open", "save", "close"]);
    let met = frame_and_answer(&mut ui, &mut waits, &[(wait_id, &from)]);
    assert_eq!(met.len(), 1, "{met:?}");
    assert_eq!(met[0].1.0, ["main/close"]);
}

#[test]
fn a_handle_is_bound_to_its_target_name_and_states_not_their_order_nor_case() {
    let (mut ui, _) = interface(&["open", "save"]);
    ui.set_target_size(
        TargetId(2),
        TargetSize::new([100, 100], 1.0).expect("a size"),
    );
    ui.frame(TargetId(2));
    let mut waits = Waits::default();
    let states = edited(&asking("button", false, None), |w| {
        w.name = "Open".to_owned();
        w.lacking = vec!["focused".to_owned(), "DISABLED".to_owned()];
    });
    let (given, after, _) = at_once(wait(&mut ui, &mut waits, &states, page(), || false));
    assert_eq!(given, ["main/open"]);
    // The same condition, its states in another order and case, its name in another case.
    let same = edited(&states, |w| {
        w.name = "open".to_owned();
        w.lacking = vec!["Disabled".to_owned(), "focused".to_owned()];
        w.after = Some(after.clone());
    });
    held(wait(&mut ui, &mut waits, &same, page(), || false));
    waits.clear(Some(&mut ui));
    for other in [
        edited(&same, |w| w.name = "save".to_owned()),
        edited(&same, |w| w.target = 2),
        edited(&same, |w| w.lacking = vec!["disabled".to_owned()]),
        edited(&same, |w| w.holding = vec!["hovered".to_owned()]),
    ] {
        let refused = wait(&mut ui, &mut waits, &other, page(), || false);
        let Err(fault) = refused else {
            panic!("another condition's handle taken: {other:?}");
        };
        assert_eq!(fault.code, Fault::BAD_PARAMS, "{other:?}");
        assert!(
            fault.message.contains("not the `after` of a wait"),
            "{}",
            fault.message
        );
    }
}

#[test]
fn a_target_that_the_interface_does_not_have_is_refused() {
    let (mut ui, _) = interface(&["open"]);
    let mut waits = Waits::default();
    let elsewhere = edited(&asking("", true, None), |w| w.target = 7);
    let Err(fault) = wait(&mut ui, &mut waits, &elsewhere, page(), || false) else {
        panic!("a wait on no target answered");
    };
    assert_eq!(fault.code, Fault::BAD_PARAMS);
    assert!(fault.message.contains("no target 7"), "{}", fault.message);
    // Given no size, the main target and one that a root stands on are the interface's.
    let mut ui = Ui::new(Fonts::new());
    let side = ui.add_root("side", RootSettings::on(TargetId(3)), |s| s);
    ui.build(side, 0, |children| {
        children.add("help", Element::button("help").style(sized), |_| {});
    });
    for target in [0, 3] {
        let there = edited(&asking("", true, None), |w| w.target = target);
        assert!(
            wait(&mut ui, &mut waits, &there, page(), || false).is_ok(),
            "target {target} refused"
        );
    }
    waits.clear(Some(&mut ui));
}

#[test]
fn the_states_are_listed_in_their_order() {
    // A wait's filter writes its states in this order, and a summary lists them so.
    assert!(State::ALL.is_sorted());
}

#[test]
fn every_role_is_named_by_its_list() {
    // Each role's place in `Role::ALL`, by a match that does not compile once a role is added:
    // give the new one the next place, and add it to `Role::ALL` by hand, which a wait reads
    // names from. This checks the list's order, not that it holds every role: Rust cannot list an
    // enumeration's cases without a macro.
    let place = |role: Role| match role {
        Role::GenericContainer => 0,
        Role::Group => 1,
        Role::Label => 2,
        Role::Image => 3,
        Role::Button => 4,
        Role::ScrollView => 5,
        Role::List => 6,
        Role::ListItem => 7,
        Role::Tree => 8,
        Role::TreeItem => 9,
        Role::Dialog => 10,
        Role::Toolbar => 11,
        Role::TextInput => 12,
        Role::SearchInput => 13,
        Role::MultilineTextInput => 14,
        Role::SpinButton => 15,
        Role::CheckBox => 16,
        Role::Slider => 17,
        Role::Details => 18,
        Role::TabList => 19,
        Role::Tab => 20,
        Role::TabPanel => 21,
        Role::Splitter => 22,
        Role::Menu => 23,
        Role::MenuBar => 24,
        Role::MenuItem => 25,
        Role::MenuItemCheckBox => 26,
        Role::ComboBox => 27,
        Role::ListBox => 28,
        Role::Pane => 29,
        Role::ToggleButton => 30,
        Role::RadioGroup => 31,
        Role::RadioButton => 32,
        Role::ProgressIndicator => 33,
        Role::Status => 34,
        Role::Alert => 35,
        Role::ListBoxOption => 36,
        Role::ColorWell => 37,
    };
    for (at, role) in Role::ALL.into_iter().enumerate() {
        assert_eq!(place(role), at, "{role:?}");
    }
}
