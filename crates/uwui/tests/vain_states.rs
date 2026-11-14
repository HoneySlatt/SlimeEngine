//! The warning that a debug build gives, once for each kind and state of the pointer, of an own
//! style that writes a hover or a press for a kind that the pointer never reaches
//! (`docs/adr/0231-an-elements-own-style-written-in-code.md`). A file of its own, its one test
//! alone in its process: `tracing` keeps whether a call site is heard across threads, which other
//! tests running beside it would change.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use slime_color::LinearColor;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};
use uwui::host::TargetId;
use uwui::text::Fonts;
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

#[test]
fn a_hover_written_for_a_kind_that_the_pointer_never_reaches_is_warned_of_once() {
    if !cfg!(debug_assertions) {
        // Only a debug build tells it.
        return;
    }
    let warnings = Arc::new(Warnings(AtomicUsize::new(0)));
    let blue = LinearColor::rgba(0.0, 0.0, 1.0, 1.0);
    tracing::subscriber::with_default(Arc::clone(&warnings), || {
        let mut ui = Ui::new(Fonts::new());
        let settings = RootSettings {
            size: [400.0, 300.0],
            scale: 1.0,
            direction: LayoutDirection::LeftToRight,
            origin: [0.0; 2],
            modal: false,
            target: uwui::host::TargetId::MAIN,
        };
        let root = ui.add_root("main", settings, |s| s);
        ui.build(root, 0, |children| {
            // A button takes the pointer: nothing to tell.
            let button = Element::button("Save").style(|s| s.hover(|s| s.background(blue)));
            children.add("button", button, |_| {});
            // Two labels and a box do not: one warning for labels' hovers, one for their presses,
            // one for a box's hover.
            for key in ["first", "second"] {
                let label = Element::label("Name").style(|s| {
                    s.hover(|s| s.text_color(blue))
                        .press(|s| s.text_color(blue))
                });
                children.add(key, label, |_| {});
            }
            let panel = Element::container().style(|s| s.hover(|s| s.background(blue)));
            children.add("box", panel, |_| {});
        });
        ui.frame(TargetId::MAIN);
        assert_eq!(warnings.0.load(Ordering::Relaxed), 3);
        ui.frame(TargetId::MAIN);
        assert_eq!(warnings.0.load(Ordering::Relaxed), 3, "not told again");
    });
}
