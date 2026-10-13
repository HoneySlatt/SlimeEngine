//! Code shared by the test programs of this directory, which each include it with `mod common;`.

use tracing::span::Id;
use tracing::{Level, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

/// A subscriber that reports each time one of the engine's spans is entered or exited, by passing
/// `sink` a line `enter <path>` or `exit <path>`. The path names the span and the engine's spans it
/// is in, outermost first, separated by `/`: `enter frame/update`. The spans of other crates, such
/// as those the windowing library records, are left out.
///
/// Set as the thread's default subscriber (`tracing::subscriber::with_default`), it takes
/// precedence on that thread over the global one that `slime_app::run` installs.
pub fn span_log(sink: impl Fn(String) + Send + Sync + 'static) -> impl Subscriber + Send + Sync {
    let engine = Targets::new()
        .with_target("slime_app", Level::TRACE)
        .with_target("slime_gfx", Level::TRACE);
    tracing_subscriber::registry()
        .with(engine)
        .with(SpanLog(sink))
}

struct SpanLog<F>(F);

impl<S, F> Layer<S> for SpanLog<F>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    F: Fn(String) + 'static,
{
    fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
        (self.0)(format!("enter {}", path(id, &ctx)));
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        (self.0)(format!("exit {}", path(id, &ctx)));
    }
}

fn path<S>(id: &Id, ctx: &Context<'_, S>) -> String
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let span = ctx
        .span(id)
        .expect("a span that is entered or exited exists");
    let names: Vec<&str> = span.scope().from_root().map(|span| span.name()).collect();
    names.join("/")
}
