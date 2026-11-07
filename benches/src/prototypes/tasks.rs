//! Whether each model cancels a task with the component that started it: a page that starts a
//! task as it opens, closed. A task here is a value that counts as running from its start until it
//! is dropped; in signals, a task of the interface's executor holds it, waiting on a timer.

use std::cell::Cell;
use std::time::Instant;

use super::SETTLE;
use super::host::{Host, Look, Model};
use super::screens::{self, Scale, Screen, screen};

thread_local! {
    /// The tasks running on this thread.
    static RUNNING: Cell<usize> = const { Cell::new(0) };
}

/// A task, running until it is dropped.
pub(crate) struct Task;

impl Task {
    pub(crate) fn start() -> Self {
        RUNNING.with(|running| running.set(running.get() + 1));
        Self
    }
}

/// A view's state cloned for a new interface starts its own task.
impl Clone for Task {
    fn clone(&self) -> Self {
        Self::start()
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        RUNNING.with(|running| running.set(running.get() - 1));
    }
}

/// A row's height, in logical pixels.
const ROW: f32 = 32.0;

mod control {
    use uwui::prelude::*;

    use super::{ROW, Task};

    /// A list whose row selected opens a page that starts a task; the page closes by Escape.
    #[derive(Clone, Default)]
    pub struct Pages {
        page: Option<usize>,
        task: Option<std::rc::Rc<Task>>,
        revision: u64,
        list: Option<ElementId>,
    }

    impl Program for Pages {
        fn build(&mut self, ui: &mut Ui, root: ElementId) {
            let list = &mut self.list;
            ui.build(root, self.revision, |children| {
                children.add("column", Element::column(), |children| {
                    let rows = Element::list(1_000, ROW).class("table").selection(0);
                    *list = Some(children.add_slot("rows", rows));
                    if let Some(row) = self.page {
                        let page = Element::label(format!("Row {row}'s page"));
                        children.add("page", page, |_| {});
                    }
                });
            });
            if let Some(list) = self.list {
                ui.build(list, 0, |children| {
                    for row in children.rows() {
                        let label = Element::label(format!("Row {row}")).class("row");
                        children.add(row, label, |_| {});
                    }
                });
            }
        }

        fn act(&mut self, _: &mut Ui, action: Action) {
            match action.kind {
                ActionKind::Intent(Intent::Accept) => {
                    self.page = Some(0);
                    self.task = Some(std::rc::Rc::new(Task::start()));
                    self.revision += 1;
                }
                ActionKind::Intent(Intent::Cancel) => {
                    // The page's elements go; nothing tells the program, which keeps its task
                    // unless it drops it here by hand.
                    self.page = None;
                    self.revision += 1;
                }
                _ => {}
            }
        }
    }
}

mod signals {
    use std::time::Duration;

    use super::super::signals::prelude::*;
    use super::{ROW, Task};

    /// A list whose row selected opens a page that starts a task, which waits an hour, cancelled
    /// with its scope.
    pub fn pages() -> impl View {
        let page = RwSignal::new(None::<usize>);
        column((
            list(1_000, ROW, |row| label(format!("Row {row}")).class("row"))
                .class("table")
                .selection(0)
                .on_accept(move || page.set(Some(0)))
                .on_cancel(move || page.set(None)),
            dyn_view(move || {
                page.get().map(|row| {
                    let task = Task::start();
                    spawn(async move {
                        let _running = task;
                        sleep(Duration::from_secs(3_600)).await;
                    });
                    label(format!("Row {row}'s page"))
                })
            }),
        ))
    }
}

mod views {
    use std::ops::Range;

    use super::super::views::prelude::*;
    use super::{ROW, Task};

    /// A list whose row selected opens a page, a view of its own that holds its task, released
    /// with its last handle as the page closes.
    #[derive(Clone, Default)]
    pub struct Pages {
        page: Option<View<Page>>,
    }

    #[derive(Clone)]
    pub struct Page {
        row: usize,
        _task: Task,
    }

    impl Render for Pages {
        fn render(&mut self, cx: &mut Cx<Self>) -> impl IntoElement {
            let rows = cx.processor(|_, rows: Range<usize>, _| {
                rows.map(|row| label(format!("Row {row}")).key(row).class("row"))
                    .collect()
            });
            let rows = list(1_000, ROW, rows)
                .class("table")
                .selection(0)
                .on_accept(cx.listener(|this, _, cx| {
                    this.page = Some(cx.new(|_| Page {
                        row: 0,
                        _task: Task::start(),
                    }));
                    cx.notify();
                }))
                .on_cancel(cx.listener(|this, _, cx| {
                    this.page = None;
                    cx.notify();
                }));
            column((rows, self.page.clone()))
        }
    }

    impl Render for Page {
        fn render(&mut self, _: &mut Cx<Self>) -> impl IntoElement {
            label(format!("Row {}'s page", self.row))
        }
    }
}

mod messages {
    use super::super::messages::prelude::*;
    use super::ROW;

    /// A list whose row selected opens a page whose task is a subscription, given up as the page
    /// closes.
    #[derive(Clone, Default)]
    pub struct Pages {
        page: Option<usize>,
    }

    #[derive(Clone, Debug)]
    pub enum Message {
        Open,
        Close,
        Tick,
    }

    impl Pages {
        pub fn update(&mut self, message: Message) {
            match message {
                Message::Open => self.page = Some(0),
                Message::Close => self.page = None,
                Message::Tick => {}
            }
        }

        pub fn view(&self) -> El<'_, Message> {
            let rows = list(1_000, ROW, |row| {
                label(format!("Row {row}")).key(row).class("row").into()
            })
            .class("table")
            .selection(0)
            .on_accept(Message::Open)
            .on_cancel(Message::Close);
            let page = self.page.map(|row| label(format!("Row {row}'s page")));
            column((rows, page)).into()
        }

        pub fn subscription(&self) -> Subscription<Message> {
            match self.page {
                Some(_) => frames().map(|_| Message::Tick),
                None => Subscription::none(),
            }
        }
    }
}

const CONTROL: Screen<control::Pages> = screen(
    screens::SELECTION,
    |_| control::Pages::default(),
    screens::page_closed,
);

const SIGNALS: Screen<super::signals::Signals<()>> = screen(
    screens::SELECTION,
    |now| super::signals::Signals::open(now, || ((), signals::pages())),
    screens::page_closed,
);

const VIEWS: Screen<super::views::Window<views::Pages>> = screen(
    screens::SELECTION,
    |now| super::views::Window::open(now, |cx| cx.new(|_| views::Pages::default())),
    screens::page_closed,
);

const MESSAGES: Screen<super::messages::Application<messages::Pages, messages::Message>> = screen(
    screens::SELECTION,
    |now| {
        super::messages::application(messages::Pages::update, messages::Pages::view)
            .subscription(messages::Pages::subscription)
            .state(messages::Pages::default, now)
    },
    screens::page_closed,
);

/// The tasks running, and the units of code that a frame without input ran, after each
/// interaction of `screen`'s script: opened, focused, a page opened, closed.
fn after_each<P: Model>(screen: &Screen<P>) -> Vec<(usize, usize)> {
    let look = Look::new(screen.sheet.0, screen.sheet.1);
    let start = Instant::now();
    let ui = look.interface().expect("the look reads");
    let mut host = Host::open(ui, look, (screen.make)(start), start);
    let mut after = Vec::new();
    for interaction in (screen.script)(Scale::Short).iter().take(4) {
        for inputs in &interaction.frames {
            host.frame(inputs);
        }
        host.settle(SETTLE);
        let still = host.frame(&[]);
        after.push((RUNNING.with(Cell::get), still.runs));
    }
    after
}

#[test]
fn the_control_keeps_a_task_that_its_program_does_not_drop() {
    let after = after_each(&CONTROL);
    assert_eq!(after[2].0, 1, "the page started its task");
    assert_eq!(after[3].0, 1, "the page closed, its task still runs");
}

#[test]
fn signals_cancel_a_task_with_its_scope() {
    let after = after_each(&SIGNALS);
    assert_eq!((after[2].0, after[3].0), (1, 0));
}

#[test]
fn an_application_closed_drops_the_tasks_that_wait() {
    let screen = &SIGNALS;
    let look = Look::new(screen.sheet.0, screen.sheet.1);
    let start = Instant::now();
    let ui = look.interface().expect("the look reads");
    let mut host = Host::open(ui, look, (screen.make)(start), start);
    for interaction in (screen.script)(Scale::Short).iter().take(3) {
        for inputs in &interaction.frames {
            host.frame(inputs);
        }
        host.settle(SETTLE);
    }
    assert_eq!(
        RUNNING.with(Cell::get),
        1,
        "the page's task waits on its timer"
    );
    drop(host);
    assert_eq!(RUNNING.with(Cell::get), 0);
}

#[test]
fn a_task_cancelled_with_its_scope_forgets_its_timer() {
    let screen = &SIGNALS;
    let look = Look::new(screen.sheet.0, screen.sheet.1);
    let start = Instant::now();
    let ui = look.interface().expect("the look reads");
    let mut host = Host::open(ui, look, (screen.make)(start), start);
    let mut deadlines = Vec::new();
    for interaction in (screen.script)(Scale::Short).iter().take(4) {
        for inputs in &interaction.frames {
            host.frame(inputs);
        }
        host.settle(SETTLE);
        deadlines.push(host.program.deadline().is_some());
    }
    assert_eq!(
        deadlines,
        [false, false, true, false],
        "the page's timer waits while it is open, and is forgotten as it closes"
    );
}

#[test]
fn views_cancel_a_task_with_the_view_that_holds_it() {
    let after = after_each(&VIEWS);
    assert_eq!((after[2].0, after[3].0), (1, 0));
}

#[test]
fn messages_give_up_a_subscription_with_the_state_that_asked_for_it() {
    let after = after_each(&MESSAGES);
    assert!(
        after[2].1 > 0,
        "while the page is open, a message at each frame"
    );
    assert_eq!(after[3].1, 0, "the page closed, none");
}
