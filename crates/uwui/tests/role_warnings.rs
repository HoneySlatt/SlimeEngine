//! The warning that a role lacking gives through `tracing`
//! (`docs/adr/0230-the-cores-roles-and-each-kinds-defaults.md`): once for each role and interface,
//! and only when themes are given. A file of its own, its one test alone in its process: `tracing`
//! keeps whether a call site is heard across threads, which other tests running beside it would
//! change.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};
use uwui::text::Fonts;
use uwui::theme::Theme;
use uwui::tree::Ui;

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
fn each_role_that_the_themes_lack_is_warned_of_once() {
    let warnings = Arc::new(Warnings(AtomicUsize::new(0)));
    let accent = Theme::from_text("accent.theme", "accent = \"#2BB673\"\n");
    tracing::subscriber::with_default(Arc::clone(&warnings), || {
        let mut ui = Ui::new(Fonts::new());
        ui.set_style(&[], &[]).expect("no theme");
        assert_eq!(
            warnings.0.load(Ordering::Relaxed),
            0,
            "no theme, no warning"
        );
        ui.set_style(&[], std::slice::from_ref(&accent))
            .expect("a theme of one role");
        // Seven roles, one given.
        assert_eq!(warnings.0.load(Ordering::Relaxed), 6, "each role lacking");
        ui.set_style(&[], std::slice::from_ref(&accent))
            .expect("the same theme");
        assert_eq!(warnings.0.load(Ordering::Relaxed), 6, "not told again");
        let mut other = Ui::new(Fonts::new());
        other
            .set_style(&[], &[accent])
            .expect("the theme, to another interface");
        assert_eq!(
            warnings.0.load(Ordering::Relaxed),
            12,
            "once for each interface"
        );
    });
}
