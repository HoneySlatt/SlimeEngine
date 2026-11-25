//! The game's end of the development channel: a run started with a token answers the tools that
//! drive it, at the start of each frame (`docs/adr/0122-the-development-channel.md`).

use std::collections::VecDeque;
use std::io::{IsTerminal as _, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, PoisonError};
use std::time::{Duration, Instant};

use slime_channel::page::{Listing, Page, Pager, cut};
use slime_channel::{
    ANNOUNCEMENT, ComponentText, Edit, EntitySummary, Event, Fault, GameState, GameTime, Incoming,
    Listener, MAX_WAITS, Reply, Request, TOKEN_ENV_VAR, Token, ViewSummary, interface,
};
use slime_gfx::{Gpu, Image, RenderTarget};
use slime_platform::KeyCode;
use slime_reflect::{Kind, Path, Registry, Value};
use slime_serialize::{Composer, Document, Mode};
use slime_world::{Command, Entity, Identities, Name, PersistentId, SceneLibrary, World};
use uwui::agents::{Answer, WaitId, Waiting, Waits};
use uwui::host::TargetId;
use uwui::tree::Ui;

use crate::game::{PickAnswer, PickSlot, Presentation, Reflected, ShownView, Simulation};
use crate::host::{Context, Host};
use crate::log_lines;

/// How many frames the measurements of the frames' times cover.
const FRAMES_MEASURED: usize = 120;

/// How long a frame's start spends taking the tools' requests, past one of each connection: the
/// rest wait for the next frame, so that a busy agent does not drop the frames of a person who
/// plays. Estimated, a little over a tenth of a frame at 240 Hz
/// (`docs/adr/0250-requests-in-flight-in-any-order-cancelled-with-an-answer-bounded-in-the-core-events-by-subscription.md`).
/// The requests that wait for a change of the simulation take as much again once it ran, past a
/// page's floor for each page asked ([`DevChannel::answer_waits`]); those that wait on the
/// interface, half as much for their replies at each of the two points where they are checked
/// ([`DevChannel::check_interface`]). What uwui reads to find them met is bounded by none: a
/// wait's first reading, which walks its target's elements, and a check, which reads what changed,
/// or walks the target whole once its marks passed their bound.
const REQUESTS_BUDGET: Duration = Duration::from_micros(500);

/// Whether the tool that started the game ended: its standard input, which the tool held, closed.
/// Set once and never cleared, as a termination signal is.
static TOOL_ENDED: AtomicBool = AtomicBool::new(false);

/// Whether the tool that started the game with a token ended, which stops the run as a termination
/// signal does ([`DevChannel::open`]).
pub(crate) fn tool_ended() -> bool {
    TOOL_ENDED.load(Ordering::Relaxed)
}

/// A run's development channel, open when the environment gave a token.
pub(crate) struct DevChannel {
    listener: Listener,
    /// The captures asked for, which the next frame drawn answers.
    captures: Vec<Incoming>,
    /// The pickings asked for, each with where the presentation gives its answer
    /// ([`Presentation::pick`]): answered once it gives it, a frame or more later.
    picks: Vec<(Incoming, PickSlot)>,
    /// The steps asked for, each with the count of steps run that answers it: answered once the
    /// frames have run them.
    steps: Vec<(Incoming, u64)>,
    /// The requests that change an editor's level or what it selects, asked for while the
    /// simulation said that they wait ([`Simulation::edits_wait`]), in their order: answered at the
    /// first frame at which it no longer does.
    edits: VecDeque<Incoming>,
    /// The requests that wait for a change, a page of the journal from its end with `wait`, which
    /// the simulation answers ([`Simulation::wait`]): asked again once each frame's simulation ran
    /// ([`DevChannel::answer_waits`]), and not counted in [`DevChannel::waiting`], so that a run at
    /// rest runs no frame for them. A cancel, or a connection that closes, wakes the run, whose
    /// next frame answers it.
    waits: Vec<Incoming>,
    /// The waits on the elements of the presentation's interface, `interface/wait`, which uwui
    /// holds, each on the window's target: checked after each point of a frame where the
    /// presentation may frame its interface ([`DevChannel::check_interface`]), and, as `waits`,
    /// not counted in [`DevChannel::waiting`]
    /// (`docs/adr/0316-the-engine-holds-the-interfaces-waits-checked-where-its-presentation-frames-it.md`).
    elements: Waits,
    /// The requests of the waits that `elements` holds, each with its name there, in the order
    /// that they came.
    on_elements: Vec<(Incoming, WaitId)>,
    /// The time between the starts of the last frames.
    frames: VecDeque<Duration>,
    /// How game time stood as the last frame's end published it, while a connection subscribed.
    published: Option<GameTime>,
    /// The listener's count of subscriptions started, as the last publication found it.
    subscriptions: u64,
}

/// Ends the run once the game's standard input closes: the tool that started it with a token holds a
/// pipe there (`slime_channel::launch`), which closes as the tool ends, crashed or not, so that no
/// game outlives it. A terminal is left alone, which a person ends the game from, and from which a
/// game run in the background would be stopped as it reads. What is read is let go, and an input
/// that is empty or missing, as `/dev/null`, ends the run at once.
fn watch_standard_input() {
    if std::io::stdin().is_terminal() {
        return;
    }
    let watching = std::thread::Builder::new()
        .name("slime dev channel input".to_owned())
        .spawn(|| {
            // `copy` reads again when a signal interrupts a read.
            let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
            tracing::info!("the tool that started the game ended: the run stops");
            TOOL_ENDED.store(true, Ordering::Relaxed);
            crate::wake::wake_current();
        });
    if let Err(error) = watching {
        tracing::warn!(%error, "the game cannot watch for the end of the tool that started it");
    }
}

impl DevChannel {
    /// Listens when the environment gives a token ([`TOKEN_ENV_VAR`]), and writes where it listens
    /// on the standard output; `None` without a token, or when it cannot listen, which the log says.
    /// Each request that comes wakes the run through `waker`.
    pub(crate) fn open(waker: crate::Waker) -> Option<Self> {
        let text = std::env::var(TOKEN_ENV_VAR).ok()?;
        let token = match text.parse::<Token>() {
            Ok(token) => token,
            Err(error) => {
                tracing::error!("the development channel does not listen: {error}");
                return None;
            }
        };
        let program = std::env::current_exe()
            .ok()
            .and_then(|path| Some(path.file_stem()?.to_string_lossy().into_owned()))
            .unwrap_or_default();
        let listener = match Listener::open(token, &program, move || waker.wake()) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(%error, "the development channel cannot listen");
                return None;
            }
        };
        let address = listener.address();
        let mut stdout = std::io::stdout().lock();
        // Whoever started the game reads it there; a closed output only leaves it unread.
        let _ = writeln!(stdout, "{ANNOUNCEMENT}{address}").and_then(|()| stdout.flush());
        tracing::info!(%address, "the development channel listens");
        watch_standard_input();
        Some(Self::listening(listener))
    }

    /// The channel of `listener`, which holds nothing yet. Unlike [`DevChannel::open`], it neither
    /// writes where it listens nor watches the standard input, whose end stops every run of the
    /// process.
    fn listening(listener: Listener) -> Self {
        Self {
            listener,
            captures: Vec::new(),
            edits: VecDeque::new(),
            waits: Vec::new(),
            elements: Waits::default(),
            on_elements: Vec::new(),
            picks: Vec::new(),
            steps: Vec::new(),
            frames: VecDeque::new(),
            published: None,
            subscriptions: 0,
        }
    }

    /// At the start of a frame that represents `real` time, after `frames` frames: answers the
    /// steps that ran, drops what waits for the frame and its tool cancelled, answering it so,
    /// answers the pickings that came and the edits that waited and no longer do, then takes the
    /// requests that arrived, one of each connection in turn, until a budget
    /// of time passed, the rest left for the next frame ([`DevChannel::waiting`]); each changes the
    /// host as a frame's start may. A step is answered once
    /// the frames have run every step asked for, so that what the tool asks next comes after them;
    /// a capture waits for the frame's image ([`DevChannel::capture`]), a picking for the
    /// answer that the presentation gives a frame or more later ([`Presentation::pick`]), a
    /// request that waits for a change of the simulation for it ([`DevChannel::answer_waits`]),
    /// and one that waits on the interface for its frame ([`DevChannel::check_interface`]); the
    /// waits that their tools cancelled are answered so first;
    /// `windowed` says whether the run has one. A save and a load are `simulation`'s
    /// ([`Simulation::save`]); the views that the last frame drew, and their settings,
    /// `presentation`'s ([`Presentation::views`]).
    pub(crate) fn answer(
        &mut self,
        host: &mut Host,
        simulation: &mut dyn Simulation,
        presentation: &mut dyn Presentation,
        frames: u64,
        real: Duration,
        windowed: bool,
    ) {
        let views = presentation.views();
        self.frames.push_back(real);
        if self.frames.len() > FRAMES_MEASURED {
            self.frames.pop_front();
        }
        let (run, asked) = (
            host.context().time().game().steps(),
            host.context().steps_asked(),
        );
        let mut waiting = Vec::new();
        for (incoming, until) in self.steps.drain(..) {
            if run >= until {
                incoming.answer(Reply::Done);
            } else if asked == 0 {
                // Game time ran again, which dropped the steps not run.
                incoming.answer(failed(
                    "game time runs again: the steps not run yet were dropped",
                ));
            } else {
                waiting.push((incoming, until));
            }
        }
        self.steps = waiting;
        // What waits and changed nothing yet, and its tool cancelled.
        for (incoming, _) in self
            .picks
            .extract_if(.., |(incoming, _)| incoming.cancelled())
        {
            incoming.cancel();
        }
        for incoming in self
            .captures
            .extract_if(.., |incoming| incoming.cancelled())
        {
            incoming.cancel();
        }
        let mut edits: Vec<_> = self.edits.drain(..).collect();
        for incoming in edits.extract_if(.., |incoming| incoming.cancelled()) {
            incoming.cancel();
        }
        self.edits = edits.into();
        // The waits cancelled, answered so in the frame that the cancel wakes, and uwui's let go.
        // A cancelled wait counts no more as it waits for this ([`DevChannel::held_of`]).
        for incoming in self.waits.extract_if(.., |incoming| incoming.cancelled()) {
            incoming.cancel();
        }
        let cancelled: Vec<_> = self
            .on_elements
            .extract_if(.., |(incoming, _)| incoming.cancelled())
            .collect();
        if !cancelled.is_empty() {
            let mut ui = presentation.ui();
            for (incoming, wait) in cancelled {
                if let Some(ui) = ui.as_deref_mut() {
                    self.elements.cancel(ui, wait);
                }
                incoming.cancel();
            }
        }
        let mut waiting = Vec::new();
        for (incoming, slot) in self.picks.drain(..) {
            // Counted before the answer is looked for: once the presentation let go of it, no
            // answer can come after.
            let alone = Arc::strong_count(&slot) == 1;
            let answer = slot.lock().unwrap_or_else(PoisonError::into_inner).take();
            match answer {
                Some(entity) => incoming.answer(picked(host.world(), entity)),
                // The presentation dropped it: no answer comes.
                None if alone => incoming.answer(failed(
                    "the game gave no answer: its GPU device was lost, or the view went away",
                )),
                None => waiting.push((incoming, slot)),
            }
        }
        self.picks = waiting;
        // The edits that waited, in their order, once they no longer do.
        while !simulation.edits_wait()
            && let Some(incoming) = self.edits.pop_front()
        {
            let reply = self.reply(
                host,
                simulation,
                presentation,
                &views,
                frames,
                &incoming,
                windowed,
                REQUESTS_BUDGET,
            );
            incoming.answer(reply);
        }
        in_rounds(
            self,
            REQUESTS_BUDGET,
            Instant::now,
            |channel| channel.listener.round(),
            |channel, incoming, share| {
                channel.take(
                    incoming,
                    host,
                    simulation,
                    presentation,
                    &views,
                    frames,
                    windowed,
                    share,
                );
            },
        );
    }

    /// Takes `incoming`, a request that a tool made: answers it, or keeps it until what it waits
    /// for comes, as [`DevChannel::answer`] says. A page of a list takes at most `share` of time,
    /// past its floor of visits.
    #[expect(
        clippy::too_many_arguments,
        reason = "what a request reaches: the host, the game's two halves, and the frame's views"
    )]
    fn take(
        &mut self,
        incoming: Incoming,
        host: &mut Host,
        simulation: &mut dyn Simulation,
        presentation: &mut dyn Presentation,
        views: &[ShownView],
        frames: u64,
        windowed: bool,
        share: Duration,
    ) {
        // An edit waits while the simulation says so, and behind those that wait already.
        if changes_an_editor(incoming.request())
            && (simulation.edits_wait() || !self.edits.is_empty())
        {
            self.edits.push_back(incoming);
            return;
        }
        if let Request::Edit { edit } = incoming.request()
            && waits(edit)
        {
            self.wait(incoming, simulation, share);
            return;
        }
        if matches!(
            incoming.request(),
            Request::Interface {
                request: interface::Request::Wait { .. }
            }
        ) {
            self.wait_on_interface(incoming, presentation, share);
            return;
        }
        if is_capture(incoming.request()) && windowed {
            self.captures.push(incoming);
            return;
        }
        if let Request::Pick { view, x, y } = incoming.request() {
            match pick(presentation, views, view, (*x, *y), windowed) {
                Ok(slot) => self.picks.push((incoming, slot)),
                Err(message) => incoming.answer(failed(message)),
            }
            return;
        }
        let reply = self.reply(
            host,
            simulation,
            presentation,
            views,
            frames,
            &incoming,
            windowed,
            share,
        );
        if matches!(incoming.request(), Request::Step { .. }) && reply == Reply::Done {
            let ctx = host.context();
            let until = ctx.time().game().steps() + u64::from(ctx.steps_asked());
            self.steps.push((incoming, until));
        } else {
            incoming.answer(reply);
        }
    }

    /// Asks `simulation` the request that waits, `incoming`, within `share` of time: answers it,
    /// or holds it while it waits, past [`MAX_WAITS`] of its connection answering it `BUSY`. A
    /// wait cancelled frees its place at once; the next frame's start answers it so, or
    /// [`DevChannel::answer_waits`] before it asks it again.
    fn wait(&mut self, incoming: Incoming, simulation: &dyn Simulation, share: Duration) {
        let Request::Edit { edit } = incoming.request() else {
            unreachable!("only an editor's requests wait");
        };
        if let Some(reply) = waited(simulation.wait(edit, incoming.page(), &mut within(share))) {
            incoming.answer(reply);
            return;
        }
        if self.held_of(&incoming) < MAX_WAITS {
            self.waits.push(incoming);
        } else {
            incoming.answer(busy());
        }
    }

    /// Asks uwui the wait on the presentation's interface that `incoming` makes, within `share` of
    /// time: refuses it on a target that the window does not draw, or without an interface;
    /// answers it when it is met now; holds it otherwise, until a check finds it met
    /// ([`DevChannel::check_interface`]), but past [`MAX_WAITS`] of its connection, where uwui lets
    /// it go at once and it is answered `BUSY`, as the journal's wait is.
    fn wait_on_interface(
        &mut self,
        incoming: Incoming,
        presentation: &mut dyn Presentation,
        share: Duration,
    ) {
        let Request::Interface { request } = incoming.request() else {
            unreachable!("only the interface's waits are asked of uwui");
        };
        let interface::Request::Wait { target, .. } = request else {
            unreachable!("only the interface's waits are asked of uwui");
        };
        // The engine draws one target into its window, as a capture says: a wait on another
        // would never end.
        if TargetId(*target) != TargetId::MAIN {
            let message = format!(
                "the game's window draws target {} alone: target {target} shows nowhere",
                TargetId::MAIN.0
            );
            incoming.answer(failed(message));
            return;
        }
        let Some(ui) = presentation.ui() else {
            incoming.answer(failed(NO_INTERFACE));
            return;
        };
        let page = incoming.page();
        match uwui::agents::wait(ui, &mut self.elements, request, page, within(share)) {
            Ok(Waiting::Reply(reply)) => incoming.answer(Reply::Interface { reply }),
            Ok(Waiting::Held(wait)) if self.held_of(&incoming) >= MAX_WAITS => {
                self.elements.cancel(ui, wait);
                incoming.answer(busy());
            }
            Ok(Waiting::Held(wait)) => self.on_elements.push((incoming, wait)),
            // A way of waiting that uwui gained and that the engine does not hold yet.
            Ok(_) => incoming.answer(Reply::from(Fault::new(
                Fault::NO_METHOD,
                "the engine does not hold this wait of the interface yet",
            ))),
            Err(fault) => incoming.answer(Reply::from(fault)),
        }
    }

    /// After a point of a frame where the presentation may have framed its interface, its
    /// extraction and its drawing (`Presentation::ui`): answers the waits on its elements that
    /// uwui finds met, within half the requests' budget parted among them, and those of them that
    /// their tools cancelled meanwhile `CANCELLED`; the other cancelled waits are answered at the
    /// next frame's start. Reads nothing while none waits, and uwui reads
    /// nothing in a frame where no element changed, nor while the interface holds a change not
    /// framed yet, which a later point of the frame, or a later frame, frames.
    pub(crate) fn check_interface(&mut self, presentation: &mut dyn Presentation) {
        if self.on_elements.is_empty() {
            return;
        }
        let Some(ui) = presentation.ui() else {
            return;
        };
        let met = self.elements.check(ui, TargetId::MAIN).met;
        if met.is_empty() {
            return;
        }
        let share = REQUESTS_BUDGET / 2 / u32::try_from(met.len()).unwrap_or(u32::MAX);
        for met in met {
            let Some(at) = self
                .on_elements
                .iter()
                .position(|(_, wait)| *wait == met.wait)
            else {
                continue;
            };
            let (incoming, _) = self.on_elements.remove(at);
            // A wait changes nothing: cancelled, it is answered so, whatever it found.
            if incoming.cancelled() {
                incoming.cancel();
                continue;
            }
            let Request::Interface { request } = incoming.request() else {
                unreachable!("only the interface's waits are asked of uwui");
            };
            let reply = uwui::agents::met(ui, request, &met, incoming.page(), within(share));
            incoming.answer(reply.map_or_else(Reply::from, |reply| Reply::Interface { reply }));
        }
    }

    /// As the run stops, before `presentation` stops: lets go the waits that uwui holds on its
    /// interface, and the readers of meaning that they hold there; the requests that wait, of both
    /// kinds, are dropped, which answers each that the game has stopped, as every request that the
    /// run holds as it ends.
    pub(crate) fn stop(&mut self, presentation: &mut dyn Presentation) {
        self.elements.clear(presentation.ui());
        self.waits.clear();
        self.on_elements.clear();
    }

    /// How many requests that wait the connection of `incoming` holds, of both kinds, but those
    /// that their tools cancelled, which the next frame's start answers so: what [`MAX_WAITS`]
    /// bounds.
    fn held_of(&self, incoming: &Incoming) -> usize {
        self.waits
            .iter()
            .chain(self.on_elements.iter().map(|(held, _)| held))
            .filter(|held| held.same_connection(incoming) && !held.cancelled())
            .count()
    }

    /// Once a frame's simulation ran, after all that may have changed what they wait for: asks
    /// `simulation` the requests that wait for a change of it again, those that ask the same page
    /// once, within the requests' budget parted among them; answers those that it answers, and
    /// holds the rest, but those that their tools cancelled, answered so. Asks nothing while none
    /// waits.
    pub(crate) fn answer_waits(&mut self, simulation: &dyn Simulation) {
        if self.waits.is_empty() {
            return;
        }
        // Those cancelled since the frame's start, answered so rather than asked.
        for incoming in self.waits.extract_if(.., |incoming| incoming.cancelled()) {
            incoming.cancel();
        }
        let held = std::mem::take(&mut self.waits);
        let share = REQUESTS_BUDGET / u32::try_from(held.len().max(1)).unwrap_or(u32::MAX);
        // Each page asked, and its answer, which the waits that ask it again share.
        let mut asked: Vec<(&Edit, slime_channel::page::Page, Option<Reply>)> = Vec::new();
        let mut replies = Vec::with_capacity(held.len());
        for incoming in &held {
            let Request::Edit { edit } = incoming.request() else {
                unreachable!("only an editor's requests wait");
            };
            let page = incoming.page();
            let reply = match asked
                .iter()
                .find(|(other, other_page, _)| *other == edit && *other_page == page)
            {
                Some((.., reply)) => reply.clone(),
                None => {
                    let reply = waited(simulation.wait(edit, page, &mut within(share)));
                    asked.push((edit, page, reply.clone()));
                    reply
                }
            };
            replies.push(reply);
        }
        drop(asked);
        for (reply, incoming) in replies.into_iter().zip(held) {
            match reply {
                Some(reply) => incoming.answer(reply),
                None => self.waits.push(incoming),
            }
        }
    }

    /// At the end of a frame, after its presentation: tells each connection subscribed to
    /// `play/time` how game time stands in `ctx`, when it changed since the last frame, or when a
    /// subscription started since, whose first value it is; builds nothing while no connection
    /// subscribed. At the end, so that what changed it in the frame, a request, the game's input,
    /// its presentation that lets go of game time, is told without a frame more
    /// (`docs/adr/0254-events-declared-in-the-welcome-replaced-in-the-carrier-published-at-the-frames-end.md`).
    pub(crate) fn publish(&mut self, ctx: &Context) {
        // The count of subscriptions read before whether one wants the event: a subscription
        // counted in it is counted in what `wants` reads.
        let subscriptions = self.listener.subscriptions();
        if !self.listener.wants(Event::TIME) {
            self.published = None;
            return;
        }
        let time = GameTime {
            paused: ctx.paused(),
            speed: ctx.speed(),
            held: ctx.held(),
        };
        if self.published != Some(time) || self.subscriptions != subscriptions {
            self.listener.publish(&Event::Time(time));
            self.published = Some(time);
            self.subscriptions = subscriptions;
        }
    }

    /// Whether a capture waits for this frame's image.
    pub(crate) fn wants_capture(&self) -> bool {
        !self.captures.is_empty()
    }

    /// Whether what a tool asked waits for the next frame: a capture, the steps asked for, a
    /// picking that the presentation answers a frame or more later, an edit that waits for the
    /// simulation, or a request that the last frame's budget left; not a request that waits for a
    /// change, which only a frame that runs for something else can bring.
    pub(crate) fn waiting(&self) -> bool {
        !self.captures.is_empty()
            || !self.steps.is_empty()
            || !self.picks.is_empty()
            || !self.edits.is_empty()
            || self.listener.waiting()
    }

    /// Answers the captures that wait with `target`, the frame's image, drawn and not yet shown, or
    /// with the part of it that one of `views` draws, or that an element of `ui` shows in.
    pub(crate) fn capture(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        views: &[ShownView],
        ui: Option<&Ui>,
    ) {
        let image = gpu.read_render_target(target);
        // The whole image, shared by every capture of it.
        let mut whole: Option<Arc<Vec<u8>>> = None;
        for incoming in self.captures.drain(..) {
            let image = match &image {
                Ok(image) => image,
                Err(error) => {
                    incoming.answer(failed(error));
                    continue;
                }
            };
            let part = match incoming.request() {
                Request::Capture { view } if view.is_empty() => {
                    let reply = Reply::Image {
                        width: image.width,
                        height: image.height,
                    };
                    let pixels = whole.get_or_insert_with(|| Arc::new(image.pixels.clone()));
                    incoming.answer_with(reply, Arc::clone(pixels));
                    continue;
                }
                Request::Capture { view } => views
                    .iter()
                    .find(|shown| shown.name == *view)
                    .cloned()
                    .ok_or_else(|| no_view(view, views)),
                Request::Interface {
                    request: interface::Request::CaptureElement { element },
                } => shown_element(ui, element),
                _ => unreachable!("only captures wait for the frame's image"),
            };
            let reply = part.and_then(|part| {
                cropped(image, &part).ok_or_else(|| {
                    format!(
                        "`{}` lies outside the image, of {} × {} pixels",
                        part.name, image.width, image.height
                    )
                })
            });
            match reply {
                Ok((width, height, pixels)) => {
                    let reply = match incoming.request() {
                        Request::Interface { .. } => Reply::Interface {
                            reply: interface::Reply::Image { width, height },
                        },
                        _ => Reply::Image { width, height },
                    };
                    incoming.answer_with(reply, Arc::new(pixels));
                }
                Err(message) => incoming.answer(failed(message)),
            }
        }
    }

    /// The reply to `incoming`'s request, a page of a list taking at most `share` of time past its
    /// floor of visits.
    #[expect(
        clippy::too_many_arguments,
        reason = "what the requests reach: the host, the game's two halves, and the frame's views"
    )]
    fn reply(
        &self,
        host: &mut Host,
        simulation: &mut dyn Simulation,
        presentation: &mut dyn Presentation,
        views: &[ShownView],
        frames: u64,
        incoming: &Incoming,
        windowed: bool,
        share: Duration,
    ) -> Reply {
        let request = incoming.request();
        let ctx = host.context_mut();
        match request {
            Request::State => Reply::State {
                state: self.state(host, presentation, views, frames),
            },
            Request::Pause => {
                ctx.pause();
                Reply::Done
            }
            Request::Resume => {
                ctx.resume();
                Reply::Done
            }
            Request::Step { steps } => {
                if !ctx.paused() {
                    return failed("game time runs: pause it first");
                }
                ctx.ask_steps(*steps);
                Reply::Done
            }
            Request::Speed { speed } => {
                if !(speed.is_finite() && *speed > 0.0) {
                    return failed(format!(
                        "{speed} is no speed: a finite number above zero is"
                    ));
                }
                ctx.set_speed(*speed);
                Reply::Done
            }
            Request::Stop => {
                ctx.request_exit();
                Reply::Done
            }
            Request::Minimize => match ctx.window() {
                Some(window) => {
                    window.minimize();
                    Reply::Done
                }
                None => failed("the game runs without a window"),
            },
            Request::Entities {
                filter,
                cursor,
                limit,
                bytes,
            } => entities(
                host.world(),
                filter,
                cursor.as_deref(),
                incoming.page().within(*limit, *bytes),
                within(share),
            )
            .unwrap_or_else(Reply::from),
            Request::Entity { entity } => entity_text(host.world(), entity).unwrap_or_else(failed),
            Request::SetProperty {
                entity,
                component,
                path,
                value,
            } => set_property(simulation, host.world_mut(), entity, component, path, value)
                .map_or_else(failed, |()| Reply::Done),
            Request::Press { key } | Request::Release { key } => {
                let Some(code) = KeyCode::from_name(key) else {
                    return failed(format!(
                        "no key `{key}`: keys are named by their position, `KeyW`, `Escape`"
                    ));
                };
                let keys = host.input_mut().keys_mut();
                if matches!(request, Request::Press { .. }) {
                    keys.press(code);
                } else {
                    keys.release(code);
                }
                Reply::Done
            }
            Request::Report { name } => {
                report(presentation, name).map_or_else(failed, |text| Reply::Report { text })
            }
            Request::SetViewSettings { view, path, value } => {
                set_view_settings(host.world(), presentation, views, view, path, value)
                    .map_or_else(failed, |()| Reply::Done)
            }
            Request::Capture { .. }
            | Request::Interface {
                request: interface::Request::CaptureElement { .. },
            } => {
                debug_assert!(!windowed, "a windowed run's capture waits for its image");
                failed("a run without a window shows no image")
            }
            Request::Interface { request } => {
                answer_interface(presentation, request, incoming.page(), within(share))
            }
            Request::Pick { .. } => unreachable!("`answer` asks the presentation to pick"),
            Request::Logs {
                after,
                limit,
                bytes,
            } => {
                let (lines, partial) =
                    log_lines::page(*after, incoming.page().within(*limit, *bytes));
                Reply::Logs { lines, partial }
            }
            Request::Save { file } => simulation
                .save(host.world(), std::path::Path::new(file))
                .map_or_else(
                    |error| failed(crate::error_chain(&*error)),
                    |()| Reply::Done,
                ),
            Request::Load { file } => simulation
                .load(host.world_mut(), std::path::Path::new(file))
                .map_or_else(
                    |error| failed(crate::error_chain(&*error)),
                    |()| Reply::Done,
                ),
            Request::Edit { edit } => simulation
                .edit(host.world_mut(), edit, incoming.page(), &mut within(share))
                .unwrap_or_else(|error| failed(crate::error_chain(&*error))),
            Request::Apply { commands } => apply(host.world_mut(), simulation, commands),
        }
    }

    /// The game's state at the start of a frame, after `frames` frames, the last of which drew
    /// `views`, whose settings `presentation` gives.
    fn state(
        &self,
        host: &Host,
        presentation: &dyn Presentation,
        views: &[ShownView],
        frames: u64,
    ) -> GameState {
        let ctx = host.context();
        let (real, game) = (ctx.time().real(), ctx.time().game());
        let milliseconds = |time: Duration| time.as_secs_f64() * 1000.0;
        let measured = self.frames.len().max(1) as f64;
        GameState {
            frame: frames,
            paused: ctx.paused(),
            held: ctx.held(),
            speed: ctx.speed(),
            steps: game.steps(),
            game_seconds: game.elapsed().as_secs_f64(),
            real_seconds: real.elapsed().as_secs_f64(),
            frame_ms: self.frames.iter().copied().map(milliseconds).sum::<f64>() / measured,
            longest_frame_ms: self
                .frames
                .iter()
                .copied()
                .map(milliseconds)
                .fold(0.0, f64::max),
            entities: host.world().len() as u64,
            views: views
                .iter()
                .map(|shown| ViewSummary {
                    name: shown.name.clone(),
                    settings: presentation
                        .view_settings(&shown.name)
                        .map(|settings| reflected_text(&settings))
                        .unwrap_or_default(),
                })
                .collect(),
            reports: presentation
                .reports()
                .into_iter()
                .map(|(name, _)| name)
                .collect(),
        }
    }
}

/// Takes the requests that `round` gives, a round at a time, and hands each to `handle` with its
/// share of what is left of `budget`, parted among the round's requests, until a round comes empty
/// or `budget` has passed since the first began, by the clock `now`: at least one round, whatever
/// its cost.
fn in_rounds<C, T>(
    context: &mut C,
    budget: Duration,
    mut now: impl FnMut() -> Instant,
    mut round: impl FnMut(&mut C) -> Vec<T>,
    mut handle: impl FnMut(&mut C, T, Duration),
) {
    let started = now();
    let mut left = budget;
    loop {
        let taken = round(context);
        if taken.is_empty() {
            return;
        }
        let share = left / u32::try_from(taken.len()).unwrap_or(u32::MAX);
        for one in taken {
            handle(context, one, share);
        }
        let spent = now().saturating_duration_since(started);
        if spent >= budget {
            return;
        }
        left = budget - spent;
    }
}

/// Whether `edit` waits for a change rather than being answered at once: a page of the journal
/// with `wait`, which [`Simulation::wait`] answers.
fn waits(edit: &Edit) -> bool {
    matches!(edit, Edit::Journal { wait: true, .. })
}

/// The reply of what [`Simulation::wait`] answered: none while it waits, a refusal saying why it
/// cannot answer.
fn waited(answered: Result<Option<Reply>, crate::AppError>) -> Option<Reply> {
    answered.unwrap_or_else(|error| Some(failed(crate::error_chain(&*error))))
}

/// The refusal of a request that waits on a connection that holds [`MAX_WAITS`] already.
fn busy() -> Reply {
    Reply::from(Fault::new(
        Fault::BUSY,
        format!(
            "the connection holds {MAX_WAITS} requests that wait already: cancel one, or wait \
             for its answer"
        ),
    ))
}

/// The reply that says why the game refused a request.
fn failed(why: impl ToString) -> Reply {
    Reply::refused(why.to_string())
}

/// Whether `share` of time has passed since now, as a page of a list asks.
fn within(share: Duration) -> impl FnMut() -> bool {
    let until = Instant::now() + share;
    move || Instant::now() >= until
}

/// Why a request that names `view` fails when the frame drew `views`, none of which is named so.
fn no_view(view: &str, views: &[ShownView]) -> String {
    format!(
        "the frame draws no view `{view}`: it draws {}",
        named(views)
    )
}

/// The names of `views`, as a message gives them.
fn named(views: &[ShownView]) -> String {
    let names: Vec<&str> = views.iter().map(|shown| shown.name.as_str()).collect();
    if names.is_empty() {
        "none by name".to_owned()
    } else {
        format!("`{}`", names.join("`, `"))
    }
}

/// `reflected` as a document in the engine's notation.
fn reflected_text(reflected: &Reflected) -> String {
    let mut composer = Composer::new(&reflected.registry);
    composer
        .put(&[], &reflected.ty, &reflected.value)
        .and_then(|()| composer.finish())
        .unwrap_or_else(|error| format!("# cannot be written: {error}"))
}

/// The report of `presentation`'s named `name`, as a document in the engine's notation.
fn report(presentation: &dyn Presentation, name: &str) -> Result<String, String> {
    let reports = presentation.reports();
    match reports.iter().find(|(named, _)| named == name) {
        Some((_, report)) => Ok(reflected_text(report)),
        None => {
            let names: Vec<&str> = reports.iter().map(|(named, _)| named.as_str()).collect();
            Err(format!(
                "the game gives no report `{name}`: it gives {}",
                if names.is_empty() {
                    "none".to_owned()
                } else {
                    format!("`{}`", names.join("`, `"))
                }
            ))
        }
    }
}

/// Sets the part at `path` of the settings of `presentation`'s view `view`, one of `views`, to
/// `text`, a value in the engine's notation, which the settings' reflection checks; `world` names
/// the entities that a value may refer to.
fn set_view_settings(
    world: &World,
    presentation: &mut dyn Presentation,
    views: &[ShownView],
    view: &str,
    path: &str,
    text: &str,
) -> Result<(), String> {
    if !views.iter().any(|shown| shown.name == view) {
        return Err(no_view(view, views));
    }
    let mut settings = presentation
        .view_settings(view)
        .ok_or_else(|| format!("the game does not give the settings of its view `{view}`"))?;
    let path: Path = path
        .parse()
        .map_err(|error| format!("`{path}` is no property path: {error}"))?;
    let ty = settings
        .registry
        .type_at(&settings.ty, &settings.value, &path)
        .map_err(|error| error.to_string())?;
    let value = read_value(&settings.registry, &ty, text, &Identities(world))?;
    settings
        .registry
        .set(&settings.ty, &mut settings.value, &path, value)
        .map_err(|error| error.to_string())?;
    presentation.set_view_settings(view, &settings)
}

/// Why a request on the interface fails in a game that has none.
const NO_INTERFACE: &str = "the game has no interface that tools read: its presentation gives \
                            none, or it runs without a window";

/// Whether `request` is a capture, which waits for the frame's image in a windowed run.
fn is_capture(request: &Request) -> bool {
    matches!(
        request,
        Request::Capture { .. }
            | Request::Interface {
                request: interface::Request::CaptureElement { .. }
            }
    )
}

/// What `presentation`'s interface answers to `request`, as uwui answers its family
/// (`docs/adr/0259-the-interfaces-family-in-uwui-agent-answered-by-uwui-an-elements-style-by-its-own-request.md`),
/// a refusal as a failure; a capture, which waits for the frame's image, is not one of them.
fn answer_interface(
    presentation: &mut dyn Presentation,
    request: &interface::Request,
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Reply {
    let Some(ui) = presentation.ui() else {
        return failed(NO_INTERFACE);
    };
    match uwui::agents::answer(ui, request, page, time_up) {
        Ok(Answer::Reply(reply)) => Reply::Interface { reply },
        Ok(Answer::Capture(_)) => unreachable!("a capture waits for the frame's image"),
        // An answer that uwui gained and that the engine does not carry yet.
        Ok(_) => Reply::from(Fault::new(
            Fault::NO_METHOD,
            "the engine does not carry this answer of the interface yet",
        )),
        Err(error) => Reply::from(error),
    }
}

/// The part of the window's image where the element at `path` of `ui` shows, or why it shows
/// nowhere: the engine draws one target into its window, `TargetId::MAIN`.
fn shown_element(ui: Option<&Ui>, path: &str) -> Result<ShownView, String> {
    let ui = ui.ok_or(NO_INTERFACE)?;
    let shown = uwui::agents::shown(ui, path).map_err(|fault| fault.message)?;
    if shown.target != TargetId::MAIN {
        return Err(format!(
            "the element `{path}` stands on target {}, which the game's window does not draw",
            shown.target.0
        ));
    }
    Ok(ShownView {
        name: path.to_owned(),
        x: shown.x,
        y: shown.y,
        width: shown.width,
        height: shown.height,
    })
}

/// The part of `image` that `view` covers, cut at the image's edges, as a presentation's views of
/// an earlier frame may pass them: its width, its height and its pixels, rows from top to bottom,
/// four bytes each; `None` when the view covers none of the image.
fn cropped(image: &Image, view: &ShownView) -> Option<(u32, u32, Vec<u8>)> {
    let right = view.x.saturating_add(view.width).min(image.width);
    let bottom = view.y.saturating_add(view.height).min(image.height);
    if view.x >= right || view.y >= bottom {
        return None;
    }
    let row_bytes = (right - view.x) as usize * 4;
    let pixels = (view.y..bottom)
        .flat_map(|row| {
            let start = (row as usize * image.width as usize + view.x as usize) * 4;
            &image.pixels[start..start + row_bytes]
        })
        .copied()
        .collect();
    Some((right - view.x, bottom - view.y, pixels))
}

/// Asks `presentation` to pick at `(x, y)` of the view named `view` among `views`, or of the whole
/// image when `view` is empty, where the last view drawn there is picked; returns where its answer
/// comes, or why it cannot pick there.
fn pick(
    presentation: &mut dyn Presentation,
    views: &[ShownView],
    view: &str,
    (x, y): (u32, u32),
    windowed: bool,
) -> Result<PickSlot, String> {
    if !windowed {
        return Err("a run without a window shows no image to pick in".to_owned());
    }
    let inside = |shown: &&ShownView| {
        (shown.x..shown.x + shown.width).contains(&x)
            && (shown.y..shown.y + shown.height).contains(&y)
    };
    let (name, at) = if view.is_empty() {
        let shown = views.iter().rev().find(inside).ok_or_else(|| {
            format!(
                "no view of the game is drawn at ({x}, {y}): it draws {}",
                named(views)
            )
        })?;
        (&shown.name, (x - shown.x, y - shown.y))
    } else {
        let shown = views
            .iter()
            .find(|shown| shown.name == view)
            .ok_or_else(|| no_view(view, views))?;
        if x >= shown.width || y >= shown.height {
            return Err(format!(
                "({x}, {y}) lies outside the view `{view}`, of {} × {} pixels",
                shown.width, shown.height
            ));
        }
        (&shown.name, (x, y))
    };
    let (answer, slot) = PickAnswer::new();
    presentation.pick(name, at, answer)?;
    Ok(slot)
}

/// The reply that names `entity` of `world`, found by a picking, or nothing: an entity destroyed
/// since it was drawn is nothing.
fn picked(world: &World, entity: Option<Entity>) -> Reply {
    match entity.filter(|&entity| world.contains(entity)) {
        Some(entity) => Reply::Picked {
            entity: summary(world, entity),
            found: true,
        },
        None => Reply::Picked {
            entity: EntitySummary {
                identity: String::new(),
                name: String::new(),
                cut: Vec::new(),
            },
            found: false,
        },
    }
}

/// The entity of `world` and its name, as a list of entities gives it.
fn summary(world: &World, entity: Entity) -> EntitySummary {
    EntitySummary {
        identity: world
            .persistent_id(entity)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        name: world
            .get::<Name>(entity)
            .map(|name| name.0.clone())
            .unwrap_or_default(),
        cut: Vec::new(),
    }
}

/// A page of the entities of `world` whose name or identity holds `filter`, whatever its case,
/// from `cursor`, none for the list's start, in the order of the world's slots: bounded by `page`
/// and `time_up`, each slot read counted as a visit, its texts cut to fit
/// (`docs/adr/0260-a-long-list-read-by-pages-a-cursor-bound-to-its-maker-pages-bounded-by-time-m6d9a1b3b-cut-in-two.md`).
fn entities(
    world: &World,
    filter: &str,
    cursor: Option<&str>,
    page: Page,
    time_up: impl FnMut() -> bool,
) -> Result<Reply, Fault> {
    let filter = filter.to_lowercase();
    let listing = Listing {
        list: "entities",
        filter: &filter,
    };
    let after = match cursor {
        None => None,
        Some(cursor) => match listing.place(cursor, page.program, world.number())?[..] {
            [] => None,
            [slot] => Some(u32::try_from(slot).map_err(|_| {
                Fault::new(
                    Fault::BAD_PARAMS,
                    format!("`{cursor}` is not a cursor of this list"),
                )
            })?),
            _ => {
                return Err(Fault::new(
                    Fault::BAD_PARAMS,
                    format!("`{cursor}` is not a cursor of this list"),
                ));
            }
        },
    };
    let mut pager = Pager::new(page, time_up);
    let mut slots = world.entities_after(after);
    let (mut entities, mut last) = (Vec::new(), after);
    while pager.visit() {
        let Some((slot, entity)) = slots.next() else {
            return Ok(Reply::Entities {
                entities,
                next: None,
            });
        };
        let Some(entity) = entity else {
            last = Some(slot);
            continue;
        };
        let mut found = summary(world, entity);
        if !(found.identity.to_lowercase().contains(&filter)
            || found.name.to_lowercase().contains(&filter))
        {
            last = Some(slot);
            continue;
        }
        for (field, text) in [("identity", &mut found.identity), ("name", &mut found.name)] {
            if cut(text, page.text_bytes()) {
                found.cut.push(field.to_owned());
            }
        }
        if !pager.take(&found) {
            // Left for the next page.
            break;
        }
        entities.push(found);
        last = Some(slot);
    }
    // The page ended before the slots did: the next starts after the last read, or from the
    // first.
    let place: Vec<u64> = last.map(u64::from).into_iter().collect();
    Ok(Reply::Entities {
        entities,
        next: Some(listing.cursor(page.program, world.number(), &place)),
    })
}

/// The entity of `world` whose persistent identity is `identity`, and that identity.
fn find(world: &World, identity: &str) -> Result<(Entity, PersistentId), String> {
    let id: PersistentId = identity
        .parse()
        .map_err(|error| format!("`{identity}` is no persistent identity: {error}"))?;
    let entity = world
        .find(&id)
        .ok_or_else(|| format!("no entity alive has the persistent identity `{identity}`"))?;
    Ok((entity, id))
}

/// The entity of `world` named `identity`, its parent and its components as text.
fn entity_text(world: &World, identity: &str) -> Result<Reply, String> {
    let (entity, _) = find(world, identity)?;
    let names = Identities(world);
    let components = world
        .component_names(entity)
        .into_iter()
        .filter_map(|name| {
            let value = world.component_value(entity, name)?;
            let mut composer = Composer::new(world.registry()).naming(&names);
            let text = composer
                .put(&[], name, &value)
                .and_then(|()| composer.finish())
                .unwrap_or_else(|error| format!("# cannot be written: {error}"));
            Some(ComponentText {
                name: name.to_owned(),
                value: text,
            })
        })
        .collect();
    let parent = world
        .parent(entity)
        .and_then(|parent| world.persistent_id(parent))
        .map(|id| id.to_string())
        .unwrap_or_default();
    Ok(Reply::Entity {
        entity: summary(world, entity),
        parent,
        components,
    })
}

/// Sets the part at `path` of the component of `world`'s entity `identity` to `text`, a value in the
/// engine's notation, as a command does at the start of a frame, through `simulation`
/// ([`Simulation::apply`]).
fn set_property(
    simulation: &mut dyn Simulation,
    world: &mut World,
    identity: &str,
    component: &str,
    path: &str,
    text: &str,
) -> Result<(), String> {
    let (entity, id) = find(world, identity)?;
    let path: Path = path
        .parse()
        .map_err(|error| format!("`{path}` is no property path: {error}"))?;
    let current = world
        .component_value(entity, component)
        .ok_or_else(|| format!("`{identity}` holds no component `{component}`"))?;
    let registry = world.registry();
    let ty_name = registry
        .get(component)
        .map(|registration| registration.info().name.clone())
        .ok_or_else(|| format!("no component type `{component}`"))?;
    let ty = registry
        .type_at(&ty_name, &current, &path)
        .map_err(|error| error.to_string())?;
    let value = read_value(registry, &ty, text, &Identities(world))?;
    let command = Command::SetProperty {
        entity: id,
        component: ty_name,
        path,
        value,
    };
    simulation
        .apply(world, &command)
        .map_err(|error| crate::error_chain(&*error))
}

/// The value of the type named `ty` that `text`, in the engine's notation, writes. An option is
/// given as its value, or as `[]` for none.
fn read_value(
    registry: &Registry,
    ty: &str,
    text: &str,
    names: &Identities<'_>,
) -> Result<Value, String> {
    let read = |ty: &str, text: &str| -> Result<Value, String> {
        let document = format!("value = {text}\n");
        Document::parse(registry, &document, Mode::Strict)
            .and_then(|mut document| document.read(&["value"], ty, Some(names)))
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "no value".to_owned())
    };
    let option = registry
        .get(ty)
        .and_then(|registration| match &registration.info().kind {
            Kind::Option(inner) => Some(inner.clone()),
            _ => None,
        });
    match option {
        // `[]`, or `[value]`, as documents write options elsewhere than in fields; else the value.
        Some(inner) => read(ty, text)
            .or_else(|_| read(&inner, text).map(|value| Value::Option(Some(Box::new(value))))),
        None => read(ty, text),
    }
}

/// Whether `request` changes an editor's level or what it selects, which waits while its simulation
/// says so ([`Simulation::edits_wait`]): an editor's requests, but those that only read, and the
/// values set and commands applied, which an editor makes steps of.
fn changes_an_editor(request: &Request) -> bool {
    match request {
        Request::Edit { edit } => !matches!(
            edit,
            Edit::History | Edit::Journal { .. } | Edit::Operations | Edit::Played
        ),
        Request::SetProperty { .. } | Request::Apply { .. } => true,
        _ => false,
    }
}

/// Applies `commands`, the texts of the world's commands, each read as `world` is when it comes,
/// through `simulation`'s library of scenes for those that place a prefab, and applied through its
/// rules ([`Simulation::apply`]), in their order (`docs/adr/0165-editing-while-playing.md`): it
/// fails at the first that cannot be read or that the rules refuse, those after it not applied.
fn apply(world: &mut World, simulation: &mut dyn Simulation, commands: &[String]) -> Reply {
    for (index, text) in commands.iter().enumerate() {
        let read = {
            let mut none = SceneLibrary::new();
            let scenes = simulation.scenes().unwrap_or(&mut none);
            Command::read(text, world, scenes)
        };
        let applied = read.map_err(|error| error.to_string()).and_then(|command| {
            simulation
                .apply(world, &command)
                .map_err(|error| crate::error_chain(&*error))
        });
        if let Err(why) = applied {
            let left = commands.len() - index - 1;
            return failed(format!(
                "command {index}: {why}; the commands after it, {left}, not applied"
            ));
        }
    }
    Reply::Done
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A view's part of the image, cut at the image's edges; none for a view beyond them.
    #[test]
    fn a_capture_of_a_view_is_its_part_of_the_image() {
        // 4 × 3 pixels, each holding its column and row.
        let image = Image {
            width: 4,
            height: 3,
            pixels: (0..3_u8)
                .flat_map(|y| (0..4_u8).flat_map(move |x| [x, y, 0, 255]))
                .collect(),
        };
        let view = |x, y, width, height| ShownView {
            name: "map".to_owned(),
            x,
            y,
            width,
            height,
        };
        let (width, height, pixels) = cropped(&image, &view(1, 1, 2, 2)).expect("inside");
        assert_eq!((width, height), (2, 2));
        assert_eq!(
            pixels,
            [
                [1, 1, 0, 255],
                [2, 1, 0, 255],
                [1, 2, 0, 255],
                [2, 2, 0, 255]
            ]
            .concat()
        );
        let (width, height, pixels) = cropped(&image, &view(3, 2, 5, 5)).expect("across");
        assert_eq!((width, height, pixels), (1, 1, vec![3, 2, 0, 255]));
        assert_eq!(cropped(&image, &view(4, 0, 2, 2)), None);
        assert_eq!(cropped(&image, &view(0, 0, 0, 2)), None);
    }

    /// The settings of the view of [`Dials`], as reflection describes them.
    #[derive(Clone, Copy, Debug, PartialEq, slime_reflect::Reflect)]
    #[reflect(name = "test::Dial")]
    struct Dial {
        /// How far it is turned.
        #[reflect(range(min = 0.0, max = 1.0))]
        level: f32,
        /// Whether it is on.
        on: bool,
    }

    /// A presentation that names a view, `dial`, whose settings a tool reads and changes, and gives
    /// them as a report too.
    struct Dials {
        dial: Dial,
    }

    impl Presentation for Dials {
        fn reports(&self) -> Vec<(String, Reflected)> {
            vec![("dial".to_owned(), Reflected::new(&self.dial))]
        }

        fn view_settings(&self, name: &str) -> Option<Reflected> {
            (name == "dial").then(|| Reflected::new(&self.dial))
        }

        fn set_view_settings(&mut self, name: &str, settings: &Reflected) -> Result<(), String> {
            assert_eq!(
                name, "dial",
                "the channel names the views that the frame drew"
            );
            self.dial = settings.get().map_err(|error| error.to_string())?;
            Ok(())
        }
    }

    /// A tool reads a view's settings as text, and sets a part of them, or the whole, within the
    /// ranges of their fields; a view that the frame did not draw, or whose settings the game does
    /// not give, is refused. A report is read by its name.
    #[test]
    fn a_tool_reads_and_changes_the_settings_of_a_view_and_reads_a_report() {
        let world = World::new();
        let mut dials = Dials {
            dial: Dial {
                level: 0.5,
                on: false,
            },
        };
        let views = ["dial", "map"].map(|name| ShownView {
            name: name.to_owned(),
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        });
        let text = reflected_text(&dials.view_settings("dial").expect("given"));
        assert!(text.contains("level = 0.5"), "{text}");
        assert!(text.contains("on = false"), "{text}");
        let mut set = |view: &str, path: &str, value: &str| {
            set_view_settings(&world, &mut dials, &views, view, path, value)
        };
        assert_eq!(set("dial", "level", "0.25"), Ok(()));
        assert_eq!(set("dial", "", "{ level = 1.0, on = true }"), Ok(()));
        // Out of its range, of another type, a part it does not have.
        for (path, value) in [("level", "1.5"), ("on", "\"yes\""), ("speed", "1.0")] {
            assert!(set("dial", path, value).is_err(), "{path} = {value}");
        }
        let refused = set("map", "level", "0.5").expect_err("not given");
        assert_eq!(
            refused,
            "the game does not give the settings of its view `map`"
        );
        let refused = set("yard", "level", "0.5").expect_err("not drawn");
        assert_eq!(
            refused,
            "the frame draws no view `yard`: it draws `dial`, `map`"
        );
        assert_eq!(
            dials.dial,
            Dial {
                level: 1.0,
                on: true
            }
        );
        // Its report, read by name.
        let text = report(&dials, "dial").expect("given");
        assert!(text.contains("level = 1.0"), "{text}");
        assert_eq!(
            report(&dials, "assets").expect_err("not given"),
            "the game gives no report `assets`: it gives `dial`"
        );
        // A presentation that keeps the default refuses every change.
        let refused = set_view_settings(&world, &mut (), &views, "dial", "level", "0.5");
        assert!(refused.is_err());
    }

    /// A presentation that picks, keeping each picking asked for, which the test answers.
    #[derive(Default)]
    struct Clicks {
        asked: Vec<(String, (u32, u32), PickAnswer)>,
    }

    impl Presentation for Clicks {
        fn pick(&mut self, view: &str, at: (u32, u32), answer: PickAnswer) -> Result<(), String> {
            self.asked.push((view.to_owned(), at, answer));
            Ok(())
        }
    }

    /// A tool picks at a pixel of a view, counted from its corner, or of the whole image, where the
    /// last view drawn there is picked; the answer names the entity once the presentation gives
    /// it, and nothing for an entity destroyed since; a pixel outside, a run without a window and a
    /// game that does not pick are refused.
    #[test]
    fn a_tool_picks_at_a_pixel_of_a_view_or_of_the_image() {
        let mut world = World::new();
        let door = world.spawn();
        world.insert(door, Name("door".to_owned()));
        let views =
            [("yard", 0, 0, 100, 50), ("map", 80, 0, 20, 20)].map(|(name, x, y, width, height)| {
                ShownView {
                    name: name.to_owned(),
                    x,
                    y,
                    width,
                    height,
                }
            });
        let mut clicks = Clicks::default();
        let in_map = pick(&mut clicks, &views, "", (85, 10), true).expect("picked");
        let in_yard = pick(&mut clicks, &views, "yard", (85, 10), true).expect("picked");
        let asked: Vec<(&str, (u32, u32))> = clicks
            .asked
            .iter()
            .map(|(view, at, _)| (view.as_str(), *at))
            .collect();
        assert_eq!(asked, [("map", (5, 10)), ("yard", (85, 10))]);
        let mut answers = std::mem::take(&mut clicks.asked)
            .into_iter()
            .map(|(_, _, answer)| answer);
        answers.next().expect("asked").give(Some(door));
        answers.next().expect("asked").give(None);
        let given = |slot: &PickSlot| slot.lock().unwrap().expect("given");
        let Reply::Picked { entity, found } = picked(&world, given(&in_map)) else {
            panic!("a picking answers with what it found");
        };
        assert!(found);
        assert_eq!(entity.name, "door");
        assert_eq!(
            picked(&world, given(&in_yard)),
            Reply::Picked {
                entity: EntitySummary {
                    identity: String::new(),
                    name: String::new(),
                    cut: Vec::new(),
                },
                found: false
            }
        );
        world.despawn(door);
        assert!(matches!(
            picked(&world, Some(door)),
            Reply::Picked { found: false, .. }
        ));
        for (view, at, windowed) in [
            ("map", (20, 0), true),
            ("", (100, 0), true),
            ("minimap", (0, 0), true),
            ("yard", (0, 0), false),
        ] {
            assert!(
                pick(&mut clicks, &views, view, at, windowed).is_err(),
                "{view} at {at:?}, windowed {windowed}"
            );
        }
        assert!(pick(&mut (), &views, "yard", (0, 0), true).is_err());
    }

    /// A presentation whose interface is a button, `open`, 100 logical pixels wide and 40 high, on
    /// the window's target, and another, `help`, on target 2, which the window does not draw.
    struct Panel {
        ui: Ui,
    }

    impl Panel {
        fn new() -> Self {
            use uwui::host::TargetSize;
            use uwui::style::Dimension;
            use uwui::text::Fonts;
            use uwui::tree::{Element, RootSettings};

            let mut ui = Ui::new(Fonts::new());
            for (key, target, button) in [
                ("main", TargetId::MAIN, "open"),
                ("side", TargetId(2), "help"),
            ] {
                let size = TargetSize::new([400, 300], 1.0).expect("a size");
                ui.set_target_size(target, size);
                let settings = RootSettings::on(target);
                let root = ui.add_root(key, settings, |s| s);
                ui.build(root, 0, |children| {
                    let sized = Element::button(button).style(|s| {
                        s.width(Dimension::Pixels(100.0))
                            .height(Dimension::Pixels(40.0))
                    });
                    children.add(button, sized, |_| {});
                });
            }
            ui.frame(TargetId::MAIN);
            ui.frame(TargetId(2));
            Self { ui }
        }
    }

    impl Presentation for Panel {
        fn ui(&mut self) -> Option<&mut Ui> {
            Some(&mut self.ui)
        }
    }

    /// The interface's family is answered by uwui, a refusal as a failure; a capture is cut from
    /// the window's image alone, which draws the window's target; a presentation without an
    /// interface refuses it all.
    #[test]
    fn the_interface_s_family_is_answered_by_uwui() {
        let mut panel = Panel::new();
        let elements = interface::Request::Elements {
            role: "button".to_owned(),
            name: String::new(),
            cursor: None,
            limit: None,
            bytes: None,
        };
        // A page as a tool of a long line asks it, whose time never passes.
        let interfaced = |presentation: &mut dyn Presentation, request: &interface::Request| {
            let page = Page::new(1, usize::MAX);
            answer_interface(presentation, request, page, || false)
        };
        let Reply::Interface {
            reply: interface::Reply::Elements {
                elements: found, ..
            },
        } = interfaced(&mut panel, &elements)
        else {
            panic!("no elements");
        };
        let paths: Vec<_> = found.iter().map(|element| element.path.as_str()).collect();
        assert_eq!(paths, ["main/open", "side/help"]);
        let press = |element: &str| interface::Request::Act {
            element: element.to_owned(),
            action: interface::Action::Press,
        };
        assert_eq!(
            interfaced(&mut panel, &press("main/open")),
            Reply::Interface {
                reply: interface::Reply::Done
            }
        );
        let Reply::Failed { code, message } = interfaced(&mut panel, &press("main/nothing")) else {
            panic!("a press on nothing done");
        };
        assert!(message.contains("no element"), "{message}");
        assert_eq!(code, Fault::REFUSED);
        // A cursor of this process, given to another.
        let first = interface::Request::Elements {
            role: String::new(),
            name: String::new(),
            cursor: None,
            limit: Some(1),
            bytes: None,
        };
        let Reply::Interface {
            reply: interface::Reply::Elements {
                next: Some(next), ..
            },
        } = interfaced(&mut panel, &first)
        else {
            panic!("a first page of one");
        };
        let rest = interface::Request::Elements {
            role: String::new(),
            name: String::new(),
            cursor: Some(next),
            limit: Some(1),
            bytes: None,
        };
        let elsewhere = Page::new(2, usize::MAX);
        let Reply::Failed { code, .. } = answer_interface(&mut panel, &rest, elsewhere, || false)
        else {
            panic!("a cursor of another process read");
        };
        assert_eq!(code, Fault::STALE, "uwui's code kept");
        let shown = shown_element(Some(&panel.ui), "main/open").expect("shown");
        assert_eq!(
            (
                shown.name.as_str(),
                shown.x,
                shown.y,
                shown.width,
                shown.height
            ),
            ("main/open", 0, 0, 100, 40)
        );
        for (path, ui, problem) in [
            ("side/help", Some(&panel.ui), "stands on target 2"),
            ("main/nothing", Some(&panel.ui), "no element"),
            ("main/open", None, "no interface"),
        ] {
            let error = shown_element(ui, path).expect_err(path);
            assert!(error.contains(problem), "{error}");
        }
        assert!(is_capture(&Request::Interface {
            request: interface::Request::CaptureElement {
                element: "main/open".to_owned()
            }
        }));
        assert!(!is_capture(&Request::Interface {
            request: elements.clone()
        }));
        assert_eq!(interfaced(&mut (), &elements), failed(NO_INTERFACE));
    }

    /// A page of `world`'s entities whose names or identities hold `filter`, from `cursor`,
    /// within `page`, whose time passed when `late`: their names and the next cursor.
    fn listed(
        world: &World,
        filter: &str,
        cursor: Option<String>,
        page: Page,
        late: bool,
    ) -> Result<(Vec<String>, Option<String>), Fault> {
        let Reply::Entities { entities, next } =
            entities(world, filter, cursor.as_deref(), page, || late)?
        else {
            panic!("not entities");
        };
        Ok((
            entities.into_iter().map(|entity| entity.name).collect(),
            next,
        ))
    }

    /// Every page of `world`'s list, `between` changing the world after each: the names listed,
    /// and how many pages gave none.
    fn every_page(
        world: &mut World,
        filter: &str,
        page: Page,
        late: bool,
        mut between: impl FnMut(&mut World, usize),
    ) -> (Vec<String>, usize) {
        let (mut names, mut empty, mut cursor) = (Vec::new(), 0, None);
        for count in 0..100_000 {
            let (found, next) = listed(world, filter, cursor, page, late).expect("a page");
            empty += usize::from(found.is_empty());
            names.extend(found);
            let Some(next) = next else {
                return (names, empty);
            };
            between(world, count);
            cursor = Some(next);
        }
        panic!("a list that does not end");
    }

    /// A world of `count` entities named `e0` and on, and their handles.
    fn named(count: usize) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let entities = (0..count)
            .map(|index| {
                let entity = world.spawn();
                world.insert(entity, Name(format!("e{index}")));
                entity
            })
            .collect();
        (world, entities)
    }

    /// Entities read by pages, while others are spawned and despawned between them: each entity
    /// alive throughout is listed once, in the order of the slots.
    #[test]
    fn entities_alive_throughout_are_listed_once_by_pages() {
        let (mut world, spawned) = named(300);
        let page = Page::new(1, usize::MAX).within(Some(7), None);
        let mut gone = Vec::new();
        let (names, _) = every_page(&mut world, "", page, false, |world, count| {
            // An entity before the cursor and one after it go; two come, in their slots.
            for at in [count * 7 % 300, (count * 7 + 150) % 300] {
                if world.despawn(spawned[at]) {
                    gone.push(format!("e{at}"));
                }
            }
            for _ in 0..2 {
                let entity = world.spawn();
                world.insert(entity, Name(format!("new{count}")));
            }
        });
        let throughout: Vec<String> = (0..300)
            .map(|index| format!("e{index}"))
            .filter(|name| !gone.contains(name))
            .collect();
        let listed: Vec<&String> = names.iter().filter(|name| name.starts_with('e')).collect();
        let once: std::collections::HashSet<&String> = listed.iter().copied().collect();
        assert_eq!(once.len(), listed.len(), "none listed twice");
        for name in &throughout {
            assert!(listed.contains(&name), "{name} listed");
        }
    }

    /// A rare match among many empty slots: pages cut short by their time find none and go on,
    /// each past the slots that it read, until the match.
    #[test]
    fn a_rare_match_among_empty_slots_is_found_past_pages_of_none() {
        let (mut world, spawned) = named(1000);
        for &entity in &spawned[..999] {
            world.despawn(entity);
        }
        let page = Page::new(1, usize::MAX);
        let (names, empty) = every_page(&mut world, "e999", page, true, |_, _| {});
        assert_eq!(names, ["e999"]);
        assert!(empty > 10, "{empty} pages of none, each a few slots");
        // The same among entities alive that the filter leaves out.
        let (mut world, _) = named(1000);
        let (names, empty) = every_page(&mut world, "e999", page, true, |_, _| {});
        assert_eq!(names, ["e999"]);
        assert!(empty > 10, "{empty} pages of none, each a few entities");
    }

    /// A page's time passes once its share has.
    #[test]
    fn a_page_s_time_passes_with_its_share() {
        assert!(within(Duration::ZERO)());
        let mut time_up = within(Duration::from_secs(3600));
        assert!(!time_up());
    }

    /// A cursor of a world that another took the place of, or of another process, is stale; one
    /// garbled or of another filter is a bad parameter; a name past a page is cut and marked.
    #[test]
    fn a_cursor_of_a_world_replaced_is_stale() {
        let (world, _) = named(10);
        let page = Page::new(1, usize::MAX).within(Some(3), None);
        let (_, next) = listed(&world, "", None, page, false).expect("a page");
        let next = next.expect("more");
        let (replaced, _) = named(10);
        let code = |world: &World, filter: &str, cursor: &str, page| {
            listed(world, filter, Some(cursor.to_owned()), page, false)
                .expect_err("refused")
                .code
        };
        assert_eq!(code(&replaced, "", &next, page), Fault::STALE);
        let elsewhere = Page::new(2, usize::MAX).within(Some(3), None);
        assert_eq!(code(&world, "", &next, elsewhere), Fault::STALE);
        assert_eq!(code(&world, "e", &next, page), Fault::BAD_PARAMS);
        assert_eq!(
            code(&world, "", &format!("{next}.1"), page),
            Fault::BAD_PARAMS
        );
        assert!(listed(&world, "", Some(next), page, false).is_ok());

        let mut world = World::new();
        let huge = world.spawn();
        world.insert(huge, Name("\u{1}".repeat(1 << 20)));
        let after = world.spawn();
        world.insert(after, Name("after".to_owned()));
        let page = Page::new(1, 64 << 10);
        let Ok(Reply::Entities {
            entities: found, ..
        }) = entities(&world, "", None, page, || false)
        else {
            panic!("entities");
        };
        assert_eq!(found.len(), 2, "past the huge name");
        assert_eq!(found[0].cut, ["name"]);
        assert!(found[0].name.len() <= page.text_bytes());
        assert_eq!((found[1].name.as_str(), found[1].cut.len()), ("after", 0));
    }

    /// The requests of a frame's start: one round at least, whatever it costs, then rounds until
    /// the budget passed, the rest left for the next frame.
    #[test]
    fn a_frame_takes_one_round_at_least_then_rounds_within_its_budget() {
        // Three connections with three requests each, a round taking one of each.
        let take = |queues: &mut Vec<Vec<u32>>| -> Vec<u32> {
            queues.iter_mut().filter_map(|queue| queue.pop()).collect()
        };
        let start = Instant::now();
        let run = |step: Duration| {
            let mut queues = vec![vec![1, 2, 3]; 3];
            let mut handled = Vec::new();
            let mut shares = Vec::new();
            let mut ticks = 0_u32;
            // Each reading of the clock, a step later.
            let clock = || {
                ticks += 1;
                start + step * ticks
            };
            in_rounds(
                &mut queues,
                Duration::from_millis(1),
                clock,
                take,
                |_, request, share| {
                    handled.push(request);
                    shares.push(share);
                },
            );
            let third = Duration::from_millis(1) / 3;
            // The first round's requests share the budget; a later round's, what is left of it.
            assert_eq!(shares[..3], [third; 3]);
            let left = Duration::from_millis(1).saturating_sub(step);
            assert!(
                shares[3..].iter().all(|&share| share == left / 3),
                "{shares:?}"
            );
            (handled.len(), queues.iter().map(Vec::len).sum::<usize>())
        };
        assert_eq!(
            run(Duration::from_secs(1)),
            (3, 6),
            "a round past the budget, then none"
        );
        assert_eq!(
            run(Duration::from_micros(600)),
            (6, 3),
            "two rounds, the second ending past the budget"
        );
        assert_eq!(run(Duration::ZERO), (9, 0), "every round within it");
    }

    /// Opens a dialog in [`Panel`]'s window, on a root of its own, which the test frames and
    /// removes.
    fn open_dialog(panel: &mut Panel) -> uwui::tree::ElementId {
        use uwui::tree::{Element, Role, RootSettings};

        let ui = &mut panel.ui;
        let root = ui.add_root("dialog", RootSettings::on(TargetId::MAIN), |s| s);
        ui.build(root, 0, |children| {
            let dialog = Element::container()
                .role(Role::Dialog)
                .named("Open a level");
            children.add("open a level", dialog, |dialog| {
                dialog.add("cancel", Element::button("Cancel"), |_| {});
            });
        });
        root
    }

    /// A simulation whose journal's waits are held until the test ends them, but a page from the
    /// journal's start, given at once.
    struct Journal;

    impl Simulation for Journal {
        fn wait(
            &self,
            edit: &Edit,
            _page: Page,
            _time_up: &mut dyn FnMut() -> bool,
        ) -> Result<Option<Reply>, crate::AppError> {
            Ok(matches!(edit, Edit::Journal { cursor: None, .. }).then_some(Reply::Done))
        }
    }

    /// A channel on a listener of the test's own, a tool connected to it, and a host: frames whose
    /// points the test runs by hand. `listening` neither prints where it listens nor watches the
    /// standard input, which `open` does for the whole process.
    struct Bench {
        channel: DevChannel,
        tool: slime_channel::Client,
        host: Host,
    }

    /// How long a test waits for what a thread of the listener does, past which it fails.
    const PATIENCE: Duration = Duration::from_secs(30);

    /// What the listener answers a request that the game drops as it stops.
    const STOPPED: &str = "the game has stopped";

    impl Bench {
        fn new() -> Self {
            let token = Token::generate().expect("a token");
            let listener =
                Listener::open(token.clone(), "slime-app's tests", || {}).expect("it listens");
            let named = slime_channel::Named::new("slime-app's tests", "");
            let tool = slime_channel::Client::connect(listener.address(), &token, &named, &named)
                .expect("connected");
            Self {
                channel: DevChannel::listening(listener),
                tool,
                host: Host::new(&crate::AppConfig::default()),
            }
        }

        /// Sends `interface/wait` with `params`, in JSON, at the core's level, as uwui's MCP
        /// library does.
        fn interface(&self, params: &str) -> slime_channel::Pending {
            let params = params.parse().expect("JSON");
            self.tool.send_call("interface/wait", params).expect("sent")
        }

        /// Sends a page of the journal that waits: from its end, which the simulation holds, or
        /// from its start, which it answers at once.
        fn journal(&self, at_the_end: bool) -> slime_channel::Pending {
            let edit = Edit::Journal {
                cursor: at_the_end.then(|| "the end".to_owned()),
                bytes: None,
                wait: true,
            };
            self.tool.send(&Request::Edit { edit }).expect("sent")
        }

        /// Waits until a request came to the listener, then runs a frame's start, which takes
        /// those that came.
        fn take(&mut self, simulation: &mut dyn Simulation, presentation: &mut dyn Presentation) {
            let until = Instant::now() + PATIENCE;
            while !self.channel.listener.waiting() {
                assert!(Instant::now() < until, "no request came");
                std::thread::sleep(Duration::from_millis(1));
            }
            self.start(simulation, presentation);
        }

        /// Runs a frame's start.
        fn start(&mut self, simulation: &mut dyn Simulation, presentation: &mut dyn Presentation) {
            let host = &mut self.host;
            self.channel
                .answer(host, simulation, presentation, 0, Duration::ZERO, true);
        }

        /// How many waits the channel holds, of the journal and of the interface.
        fn held(&self) -> (usize, usize) {
            (self.channel.waits.len(), self.channel.on_elements.len())
        }

        /// Waits until the tool's cancel of the interface's wait held at `at` reached the
        /// listener.
        fn until_cancelled(&self, at: usize) {
            self.until(|channel| channel.on_elements[at].0.cancelled());
        }

        /// Waits until `holds` holds of the channel, as a thread of the listener makes it.
        fn until(&self, holds: impl Fn(&DevChannel) -> bool) {
            let until = Instant::now() + PATIENCE;
            while !holds(&self.channel) {
                assert!(Instant::now() < until, "the cancel never came");
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }

    /// The parameters of a wait on the role Dialog, named `Open a level`, present or `absent`,
    /// after `after`, on `target`.
    fn dialog_wait(absent: bool, after: Option<&str>, target: u32) -> String {
        let after = after.map_or_else(String::new, |after| format!(r#", "after": "{after}""#));
        format!(
            r#"{{ "target": {target}, "role": "Dialog", "name": "open a level", "absent": {absent}{after} }}"#
        )
    }

    /// The answer of `pending`, a request of the core's level, within [`PATIENCE`]: the client's
    /// own bound is ten minutes.
    fn outcome(pending: slime_channel::Pending) -> slime_channel::Outcome {
        let (sent, answered) = std::sync::mpsc::channel();
        std::thread::spawn(move || sent.send(pending.wait_outcome()));
        answered
            .recv_timeout(PATIENCE)
            .expect("answered within the test's patience")
    }

    /// The paths of the elements and the `after` of a wait's answer.
    fn waited(pending: slime_channel::Pending) -> (Vec<String>, String) {
        let reply = match outcome(pending) {
            Ok((Ok(reply), _)) => reply,
            other => panic!("no wait met: {other:?}"),
        };
        assert_eq!(reply["reply"], "waited", "{reply}");
        let paths = reply["elements"]
            .as_array()
            .expect("elements")
            .iter()
            .map(|element| element["path"].as_str().expect("a path").to_owned())
            .collect();
        (paths, reply["after"].as_str().expect("after").to_owned())
    }

    /// The fault of a request's answer.
    fn fault_of(outcome: slime_channel::Outcome) -> Fault {
        match outcome {
            Ok((Err(fault), _)) => fault,
            other => panic!("no fault: {other:?}"),
        }
    }

    /// The reply of a request of the channel's own, within [`PATIENCE`].
    fn replied(pending: slime_channel::Pending) -> Result<Reply, slime_channel::ChannelError> {
        pending.wait_for(PATIENCE).map(|(reply, _)| reply)
    }

    /// The code and the message of a refusal of the channel's own.
    fn refused(pending: slime_channel::Pending) -> (i64, String) {
        match replied(pending) {
            Ok(Reply::Failed { code, message }) => (code, message),
            other => panic!("not refused: {other:?}"),
        }
    }

    /// Whether `ui` gives as many readers of meaning as an interface holds: none is left by waits.
    fn readers_free(ui: &mut Ui) -> bool {
        (0..uwui::tree::MEANING_READERS).all(|_| ui.add_meaning_reader(TargetId::MAIN).is_some())
    }

    /// A wait on the window's interface is held by uwui while what it waits for has not come, a
    /// change not framed yet reading nothing, and answered by the check after the frame that shows
    /// it: a dialog opened, then closed, seen once; a wait cancelled before its check is answered
    /// so, whatever the check found; the run's end drops what waits, and its readers.
    #[test]
    fn a_wait_on_the_interface_is_answered_by_the_check_after_its_frame_and_seen_once() {
        let (mut bench, mut panel) = (Bench::new(), Panel::new());
        let opened = bench.interface(&dialog_wait(false, None, 0));
        bench.take(&mut (), &mut panel);
        assert_eq!(bench.held(), (0, 1), "held: no dialog yet");
        // A still frame reads nothing, and a change not yet framed nothing either.
        bench.channel.check_interface(&mut panel);
        let dialog = open_dialog(&mut panel);
        bench.channel.check_interface(&mut panel);
        assert_eq!(bench.held(), (0, 1), "the dialog is not framed yet");
        panel.ui.frame(TargetId::MAIN);
        bench.channel.check_interface(&mut panel);
        assert_eq!(bench.held(), (0, 0));
        let (elements, after) = waited(opened);
        assert_eq!(elements, ["dialog/open a level"]);
        // Its absence, from that answer, held until it closes.
        let closed = bench.interface(&dialog_wait(true, Some(&after), 0));
        bench.take(&mut (), &mut panel);
        assert_eq!(bench.held(), (0, 1), "held: the dialog stands");
        panel.ui.remove(dialog);
        panel.ui.frame(TargetId::MAIN);
        bench.channel.check_interface(&mut panel);
        let (elements, after) = waited(closed);
        assert!(elements.is_empty(), "{elements:?}");
        // Seen once: from that answer, the absence is held.
        let again = bench.interface(&dialog_wait(true, Some(&after), 0));
        bench.take(&mut (), &mut panel);
        panel.ui.frame(TargetId::MAIN);
        bench.channel.check_interface(&mut panel);
        assert_eq!(bench.held(), (0, 1), "seen once");
        // Cancelled, then met in the same frame: answered cancelled.
        let reopened = bench.interface(&dialog_wait(false, Some(&after), 0));
        bench.take(&mut (), &mut panel);
        assert_eq!(bench.held(), (0, 2));
        reopened.cancel();
        bench.until_cancelled(1);
        open_dialog(&mut panel);
        panel.ui.frame(TargetId::MAIN);
        bench.channel.check_interface(&mut panel);
        assert_eq!(fault_of(outcome(reopened)).code, Fault::CANCELLED);
        assert_eq!(bench.held(), (0, 1));
        // The run's end drops it, which answers it, and lets its reader of the interface go.
        bench.channel.stop(&mut panel);
        assert_eq!(bench.held(), (0, 0));
        let fault = fault_of(outcome(again));
        assert!(fault.message.contains(STOPPED), "{}", fault.message);
        assert!(readers_free(&mut panel.ui), "a reader left");
    }

    /// A simulation that answers every page that waits.
    struct Answering;

    impl Simulation for Answering {
        fn wait(
            &self,
            _edit: &Edit,
            _page: Page,
            _time_up: &mut dyn FnMut() -> bool,
        ) -> Result<Option<Reply>, crate::AppError> {
            Ok(Some(Reply::Done))
        }
    }

    /// A journal's wait that its tool cancelled after the frame's start is answered cancelled
    /// when the simulation is asked it again, whatever it would answer: a wait changes nothing.
    #[test]
    fn a_journal_s_wait_cancelled_during_its_frame_is_answered_cancelled() {
        let mut bench = Bench::new();
        let waiting = bench.journal(true);
        bench.take(&mut Journal, &mut ());
        assert_eq!(bench.held(), (1, 0));
        waiting.cancel();
        bench.until(|channel| channel.waits[0].cancelled());
        bench.channel.answer_waits(&Answering);
        assert!(matches!(
            replied(waiting),
            Err(slime_channel::ChannelError::Cancelled)
        ));
        assert_eq!(bench.held(), (0, 0));
    }

    /// A wait on the interface that its tool cancelled is answered so at the next frame's start,
    /// and uwui lets it go: no reader of the interface is left before the run ends.
    #[test]
    fn a_wait_on_the_interface_cancelled_is_let_go_at_the_frame_s_start() {
        let (mut bench, mut panel) = (Bench::new(), Panel::new());
        let waiting = bench.interface(&dialog_wait(false, None, 0));
        bench.take(&mut (), &mut panel);
        assert_eq!(bench.held(), (0, 1));
        waiting.cancel();
        bench.until_cancelled(0);
        bench.start(&mut (), &mut panel);
        assert_eq!(fault_of(outcome(waiting)).code, Fault::CANCELLED);
        assert_eq!(bench.held(), (0, 0));
        assert!(readers_free(&mut panel.ui), "uwui holds the wait cancelled");
    }

    /// A wait on the interface that would be held past [`MAX_WAITS`] is `BUSY`, and uwui holds
    /// nothing of it: no reader of the interface is left.
    #[test]
    fn a_wait_on_the_interface_past_the_count_holds_nothing() {
        let (mut bench, mut panel) = (Bench::new(), Panel::new());
        let journal: Vec<_> = (0..MAX_WAITS).map(|_| bench.journal(true)).collect();
        let until = Instant::now() + PATIENCE;
        while bench.held() != (MAX_WAITS, 0) {
            assert!(Instant::now() < until, "held {:?}", bench.held());
            bench.start(&mut Journal, &mut panel);
            std::thread::sleep(Duration::from_millis(1));
        }
        let busy = bench.interface(&dialog_wait(false, None, 0));
        bench.take(&mut Journal, &mut panel);
        assert_eq!(fault_of(outcome(busy)).code, Fault::BUSY);
        assert!(readers_free(&mut panel.ui), "uwui holds the wait refused");
        drop(journal);
    }

    /// A wait on a target that the window does not draw, on a game without an interface, or for a
    /// role that the family does not name, is refused, saying why; none is held.
    #[test]
    fn a_wait_on_another_target_or_without_an_interface_is_refused() {
        let (mut bench, mut panel) = (Bench::new(), Panel::new());
        let side = bench.interface(&dialog_wait(false, None, 2));
        bench.take(&mut (), &mut panel);
        let fault = fault_of(outcome(side));
        assert_eq!(fault.code, Fault::REFUSED, "{}", fault.message);
        assert!(
            fault.message.contains("draws target 0 alone"),
            "{}",
            fault.message
        );
        let none = bench.interface(&dialog_wait(false, None, 0));
        bench.take(&mut (), &mut ());
        assert_eq!(fault_of(outcome(none)).message, NO_INTERFACE);
        let unknown = bench.interface(r#"{ "target": 0, "role": "Dialogue", "name": "" }"#);
        bench.take(&mut (), &mut panel);
        assert_eq!(fault_of(outcome(unknown)).code, Fault::BAD_PARAMS);
        assert_eq!(bench.held(), (0, 0));
        assert!(readers_free(&mut panel.ui), "a reader left");
    }

    /// The journal's waits and the interface's share a connection's count: past [`MAX_WAITS`], a
    /// wait of either kind that would be held is `BUSY`, uwui's let go, and one met at once is
    /// answered; a cancel that came before a new wait frees its place in the frame that takes
    /// both, of either kind, and a cancel not yet answered counts no more; the run's end drops
    /// every wait held, which answers it.
    #[test]
    fn waits_of_both_kinds_share_a_count_which_a_cancel_frees_in_its_frame() {
        let (mut bench, mut panel) = (Bench::new(), Panel::new());
        let mut journal: Vec<_> = (0..MAX_WAITS / 2).map(|_| bench.journal(true)).collect();
        let mut on_interface: Vec<_> = (MAX_WAITS / 2..MAX_WAITS)
            .map(|_| bench.interface(&dialog_wait(false, None, 0)))
            .collect();
        let until = Instant::now() + PATIENCE;
        while bench.held() != (MAX_WAITS / 2, MAX_WAITS - MAX_WAITS / 2) {
            assert!(Instant::now() < until, "held {:?}", bench.held());
            bench.start(&mut Journal, &mut panel);
            std::thread::sleep(Duration::from_millis(1));
        }
        // Full: a wait of either kind that would be held is refused, one met at once answered.
        let busy = bench.journal(true);
        bench.take(&mut Journal, &mut panel);
        assert_eq!(refused(busy).0, Fault::BUSY);
        let busy = bench.interface(&dialog_wait(false, None, 0));
        bench.take(&mut Journal, &mut panel);
        assert_eq!(fault_of(outcome(busy)).code, Fault::BUSY);
        let at_once = bench.journal(false);
        bench.take(&mut Journal, &mut panel);
        assert_eq!(replied(at_once).expect("answered"), Reply::Done);
        let at_once = bench.interface(r#"{ "target": 0, "role": "Button", "name": "open" }"#);
        bench.take(&mut Journal, &mut panel);
        assert_eq!(waited(at_once).0, ["main/open"]);
        assert_eq!(bench.held(), (MAX_WAITS / 2, MAX_WAITS / 2));
        // A journal's wait cancelled, then a wait on the interface, taken in one frame.
        let cancelled = journal.remove(0);
        cancelled.cancel();
        on_interface.push(bench.interface(&dialog_wait(false, None, 0)));
        bench.take(&mut Journal, &mut panel);
        assert!(matches!(
            replied(cancelled),
            Err(slime_channel::ChannelError::Cancelled)
        ));
        assert_eq!(bench.held(), (MAX_WAITS / 2 - 1, MAX_WAITS / 2 + 1));
        // And the other way.
        let cancelled = on_interface.remove(0);
        cancelled.cancel();
        journal.push(bench.journal(true));
        bench.take(&mut Journal, &mut panel);
        assert_eq!(fault_of(outcome(cancelled)).code, Fault::CANCELLED);
        assert_eq!(bench.held(), (MAX_WAITS / 2, MAX_WAITS / 2));
        // A cancel that came after the frame's start, not answered yet, counts no more.
        on_interface[0].cancel();
        bench.until_cancelled(0);
        let late = bench.journal(true);
        let until = Instant::now() + PATIENCE;
        let incoming = loop {
            if let Some(incoming) = bench.channel.listener.next() {
                break incoming;
            }
            assert!(Instant::now() < until, "the wait never came");
            std::thread::sleep(Duration::from_millis(1));
        };
        let views = Vec::new();
        bench.channel.take(
            incoming,
            &mut bench.host,
            &mut Journal,
            &mut panel,
            &views,
            0,
            true,
            REQUESTS_BUDGET,
        );
        assert_eq!(bench.held(), (MAX_WAITS / 2 + 1, MAX_WAITS / 2));
        // The run's end drops every wait held, of both kinds, which answers each.
        bench.channel.stop(&mut panel);
        assert_eq!(bench.held(), (0, 0));
        for pending in journal.into_iter().chain([late]) {
            assert_eq!(refused(pending).1, STOPPED);
        }
        for pending in on_interface {
            let fault = fault_of(outcome(pending));
            assert!(fault.message.contains(STOPPED), "{}", fault.message);
        }
        assert!(readers_free(&mut panel.ui), "a reader left");
    }
}
