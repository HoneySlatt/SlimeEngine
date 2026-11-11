//! The warnings that tokens give through `tracing`
//! (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`): a token that the themes
//! lack, once for each token and interface, only when themes are given; a token that an own style
//! names and no list declared, once for each token and interface. A file of its own, its one test
//! alone in its process: `tracing` keeps whether a call site is heard across threads, which other
//! tests running beside it would change.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};
use uwui::text::Fonts;
use uwui::theme::{Fallback, Length, Theme, Token, Tokens};
use uwui::tree::{Element, LayoutDirection, RootSettings, Ui};

/// A subscriber that counts the warnings given while it is the default.
struct Warnings(AtomicUsize);

impl Subscriber for Warnings {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}

    fn record_follows_from(&self, _: &Id, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if *event.metadata().level() == Level::WARN {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn enter(&self, _: &Id) {}

    fn exit(&self, _: &Id) {}
}

const GAP: Token<Length> = Token::new("test_gap", Fallback::Value(5.0));
const LOOSE: Token<Length> = Token::new("test_loose", Fallback::Value(7.0));
const TOKENS: Tokens = Tokens::new(&[&GAP]);

/// The core's seven roles, and `test_gap`, given: no token lacking.
const GIVEN: &str = "canvas_text = \"#FFFFFF\"\ngray_text = \"#888888\"\nbutton_face = \"#444444\"\n\
                     accent = \"#4C8DFF\"\nhighlight = \"#4C8DFF\"\nhighlight_text = \"#FFFFFF\"\n\
                     focus = \"#4C8DFF\"\n";

fn count(warnings: &Warnings) -> usize {
    warnings.0.load(Ordering::Relaxed)
}

/// Gives `ui` a root of `elements` children whose own styles name `test_gap` and `test_loose`, and
/// makes a frame.
fn with_elements(ui: &mut Ui, elements: usize) {
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    let root = ui.add_root("main", settings, |s| s);
    ui.build(root, 0, |children| {
        for index in 0..elements {
            // Each its own growth, so that each makes its own style.
            let element = Element::container()
                .style(move |s| s.padding(GAP).margin(LOOSE).grow(index as f32));
            children.add(index, element, |_| {});
        }
    });
    ui.frame();
}

/// Gives `ui` a root whose own style names `test_gap` alone, and makes a frame.
fn with_the_gap(ui: &mut Ui) {
    let settings = RootSettings {
        size: [100.0, 100.0],
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        origin: [0.0; 2],
        modal: false,
    };
    ui.add_root("main", settings, |s| s.padding(GAP));
    ui.frame();
}

#[test]
fn a_token_lacking_and_a_token_undeclared_are_each_warned_of_once() {
    let warnings = Arc::new(Warnings(AtomicUsize::new(0)));
    let roles = Theme::from_text("roles.theme", GIVEN);
    tracing::subscriber::with_default(Arc::clone(&warnings), || {
        let mut ui = Ui::new(Fonts::new());
        ui.declare_tokens(&TOKENS).expect("a name of its own");
        ui.set_style(&[], &[]).expect("no theme");
        assert_eq!(
            count(&warnings),
            0,
            "no theme, no warning of a token lacking"
        );
        ui.set_style(&[], std::slice::from_ref(&roles))
            .expect("the roles");
        assert_eq!(count(&warnings), 1, "`test_gap` lacking");
        ui.set_style(&[], std::slice::from_ref(&roles))
            .expect("the same themes");
        assert_eq!(count(&warnings), 1, "not told again");
        with_elements(&mut ui, 3);
        assert_eq!(
            count(&warnings),
            2,
            "`test_loose` undeclared, once for three elements"
        );
        let mut other = Ui::new(Fonts::new());
        other.declare_tokens(&TOKENS).expect("a name of its own");
        other.set_style(&[], &[roles]).expect("the roles");
        with_elements(&mut other, 1);
        assert_eq!(count(&warnings), 4, "once for each interface");
        // A list declared after the themes, or to an interface that is given none, is declared:
        // its tokens take their fallbacks, untold, until the themes are given again.
        let mut late = Ui::new(Fonts::new());
        late.set_style(&[], &[]).expect("no theme");
        late.declare_tokens(&TOKENS).expect("a name of its own");
        with_the_gap(&mut late);
        let mut unstyled = Ui::new(Fonts::new());
        unstyled.declare_tokens(&TOKENS).expect("a name of its own");
        with_the_gap(&mut unstyled);
        assert_eq!(count(&warnings), 4, "a token declared is never undeclared");
    });
}
