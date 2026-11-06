//! The signals' runtime, written by hand for the prototypes, as Floem's works: on one thread, a
//! graph of signals, memos worked out from them, and bindings, the closures that the view tracks,
//! each owned by a scope that frees them when it is disposed. A read inside a memo or a binding
//! subscribes it; a signal that changes marks its memos stale and queues its bindings, which
//! `flush` runs, the memos first, a memo telling its own only when its value changed.
//!
//! Its two panics are those of Floem: a value borrowed twice, its `RefCell`'s, and a signal of a
//! scope disposed, read.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

/// A node of the graph: its place in the arena, and the generation of that place, so that the id
/// of a node disposed names no other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Id {
    index: u32,
    generation: u32,
}

/// A value of the graph, whatever its type.
pub(crate) type Value = Rc<RefCell<dyn Any>>;

/// What clones a signal's value, for a new interface built from the same state.
type Cloner = fn(&dyn Any) -> Box<dyn Any>;

enum Kind {
    /// A signal or a value stored, cloned by its cloner.
    Signal(Cloner),
    /// A memo: what works its value out again, and tells whether it changed.
    Memo(Rc<dyn Fn() -> bool>),
    /// A binding: what runs again when what it read changes.
    Binding(Rc<dyn Fn()>),
    Scope,
}

struct Node {
    kind: Kind,
    value: Option<Value>,
    /// The memos and bindings that read it.
    subscribers: Vec<Id>,
    /// What it read, for a memo or a binding.
    sources: Vec<Id>,
    /// The scope that owns it.
    owner: Option<Id>,
    /// What it owns, for a scope.
    owned: Vec<Id>,
    /// What runs when it is disposed.
    cleanups: Vec<Box<dyn FnOnce()>>,
    /// For a memo, whether its value must be worked out again before it is read.
    stale: bool,
    /// For a binding, whether it waits to run.
    queued: bool,
}

struct Slot {
    generation: u32,
    node: Option<Node>,
}

/// A graph of signals, of one application.
pub(crate) struct Runtime {
    slots: RefCell<Vec<Slot>>,
    free: RefCell<Vec<u32>>,
    /// The memo or binding that runs, which what is read subscribes.
    observer: Cell<Option<Id>>,
    /// The scope that owns what is made.
    owner: Cell<Option<Id>>,
    stale: RefCell<VecDeque<Id>>,
    queue: RefCell<VecDeque<Id>>,
    /// Memos worked out and bindings run again, since last counted.
    pub(crate) runs: Cell<usize>,
    root: Id,
    /// The signals and values stored that the root scope made, in their order: the state that a
    /// new interface is built from.
    made: RefCell<Vec<Id>>,
    /// The values that the root scope's signals take as they are made, in their order, for a new
    /// interface built from the same state: none past them.
    replay: RefCell<VecDeque<Box<dyn Any>>>,
    /// The signal of each frame's time.
    pub(crate) clock: Cell<Option<Id>>,
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Runtime>>> = const { RefCell::new(None) };
}

/// The runtime of the application that runs on this thread.
///
/// # Panics
///
/// Outside an application's runtime ([`enter`]).
pub(crate) fn current() -> Rc<Runtime> {
    CURRENT.with(|current| {
        current
            .borrow()
            .clone()
            .expect("a signal used within an application")
    })
}

/// Runs `run` with `runtime` as the thread's, and gives the one before back after, a panic too.
pub(crate) fn enter<R>(runtime: &Rc<Runtime>, run: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<Runtime>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|current| *current.borrow_mut() = self.0.take());
        }
    }
    let before = CURRENT.with(|current| current.borrow_mut().replace(Rc::clone(runtime)));
    let _restore = Restore(before);
    run()
}

impl Runtime {
    /// A runtime whose root scope's signals take `replay`'s values, in their order, as they are
    /// made.
    pub(crate) fn new(replay: VecDeque<Box<dyn Any>>) -> Rc<Self> {
        let root = Id {
            index: 0,
            generation: 0,
        };
        Rc::new(Self {
            slots: RefCell::new(vec![Slot {
                generation: 0,
                node: Some(Node::new(Kind::Scope, None, None)),
            }]),
            free: RefCell::new(Vec::new()),
            observer: Cell::new(None),
            owner: Cell::new(Some(root)),
            stale: RefCell::new(VecDeque::new()),
            queue: RefCell::new(VecDeque::new()),
            runs: Cell::new(0),
            root,
            made: RefCell::new(Vec::new()),
            replay: RefCell::new(replay),
            clock: Cell::new(None),
        })
    }

    /// The values of the signals that the root scope made, in their order, cloned.
    pub(crate) fn snapshot(&self) -> VecDeque<Box<dyn Any>> {
        let slots = self.slots.borrow();
        self.made
            .borrow()
            .iter()
            .filter_map(|id| {
                let node = slots[id.index as usize]
                    .node
                    .as_ref()
                    .filter(|_| slots[id.index as usize].generation == id.generation)?;
                let Kind::Signal(clone) = node.kind else {
                    return None;
                };
                Some(clone(&*node.value.as_ref()?.borrow()))
            })
            .collect()
    }

    fn insert(&self, node: Node) -> Id {
        let owner = node.owner;
        let mut slots = self.slots.borrow_mut();
        let id = match self.free.borrow_mut().pop() {
            Some(index) => {
                let slot = &mut slots[index as usize];
                slot.node = Some(node);
                Id {
                    index,
                    generation: slot.generation,
                }
            }
            None => {
                slots.push(Slot {
                    generation: 0,
                    node: Some(node),
                });
                Id {
                    index: (slots.len() - 1) as u32,
                    generation: 0,
                }
            }
        };
        if let Some(owner) = owner
            && let Some(owner) = node_mut(&mut slots, owner)
        {
            owner.owned.push(id);
        }
        id
    }

    /// A signal of `value`, or, made by the root scope, of the value that it replays.
    pub(crate) fn signal<T: Clone + 'static>(&self, value: T) -> Id {
        let owner = self.owner.get();
        let value = if owner == Some(self.root) {
            let replayed = self.replay.borrow_mut().pop_front();
            replayed
                .and_then(|replayed| replayed.downcast::<T>().ok())
                .map_or(value, |replayed| *replayed)
        } else {
            value
        };
        let cell: Value = Rc::new(RefCell::new(value));
        let clone: Cloner = |value| {
            let value = value.downcast_ref::<T>().expect("a value of its type");
            Box::new(value.clone())
        };
        let id = self.insert(Node::new(Kind::Signal(clone), Some(cell), owner));
        if owner == Some(self.root) {
            self.made.borrow_mut().push(id);
        }
        id
    }

    /// A memo, whose value `cell` holds, worked out again by `compute`, which tells whether it
    /// changed; stale until first read.
    pub(crate) fn memo(&self, cell: Value, compute: Rc<dyn Fn() -> bool>) -> Id {
        let mut node = Node::new(Kind::Memo(compute), Some(cell), self.owner.get());
        node.stale = true;
        self.insert(node)
    }

    /// A binding that runs `run` now, and again when what it read changes.
    pub(crate) fn binding(&self, run: Rc<dyn Fn()>) -> Id {
        let id = self.insert(Node::new(
            Kind::Binding(Rc::clone(&run)),
            None,
            self.owner.get(),
        ));
        self.run_tracked(id, &*run);
        id
    }

    /// A scope, owned by the scope that owns what is made now.
    pub(crate) fn scope(&self) -> Id {
        self.insert(Node::new(Kind::Scope, None, self.owner.get()))
    }

    /// The root scope.
    pub(crate) fn root(&self) -> Id {
        self.root
    }

    /// Runs `run` with `scope` owning what it makes.
    pub(crate) fn owned_by<R>(&self, scope: Id, run: impl FnOnce() -> R) -> R {
        let outer = self.owner.replace(Some(scope));
        let result = run();
        self.owner.set(outer);
        result
    }

    /// Runs `run` with no memo nor binding subscribing what it reads.
    pub(crate) fn untracked<R>(&self, run: impl FnOnce() -> R) -> R {
        let outer = self.observer.replace(None);
        let result = run();
        self.observer.set(outer);
        result
    }

    /// What runs when the scope that owns what is made now is disposed.
    pub(crate) fn on_cleanup(&self, cleanup: impl FnOnce() + 'static) {
        let Some(owner) = self.owner.get() else {
            return;
        };
        if let Some(owner) = node_mut(&mut self.slots.borrow_mut(), owner) {
            owner.cleanups.push(Box::new(cleanup));
        }
    }

    /// The value of `id`, which the memo or binding that runs now subscribes; a memo's worked
    /// out first if stale.
    ///
    /// # Panics
    ///
    /// If `id`'s scope was disposed.
    pub(crate) fn read(&self, id: Id) -> Value {
        let stale = {
            let slots = self.slots.borrow();
            let node = node(&slots, id).expect("a signal of a scope not disposed");
            node.stale
        };
        if stale {
            self.work_out(id);
        }
        if let Some(observer) = self.observer.get() {
            self.subscribe(observer, id);
        }
        self.value(id)
    }

    /// The value of `id`, which nothing subscribes.
    ///
    /// # Panics
    ///
    /// If `id`'s scope was disposed.
    pub(crate) fn value(&self, id: Id) -> Value {
        let slots = self.slots.borrow();
        let node = node(&slots, id).expect("a signal of a scope not disposed");
        Rc::clone(node.value.as_ref().expect("a node that holds a value"))
    }

    /// Whether a memo or a binding reads `id`.
    pub(crate) fn read_by_any(&self, id: Id) -> bool {
        let slots = self.slots.borrow();
        node(&slots, id).is_some_and(|node| !node.subscribers.is_empty())
    }

    fn subscribe(&self, observer: Id, source: Id) {
        let mut slots = self.slots.borrow_mut();
        let Some(node) = node_mut(&mut slots, source) else {
            return;
        };
        if node.subscribers.contains(&observer) {
            return;
        }
        node.subscribers.push(observer);
        if let Some(observer) = node_mut(&mut slots, observer) {
            observer.sources.push(source);
        }
    }

    /// Forgets what `id` read, before it reads again.
    fn unsubscribe(&self, id: Id) {
        let mut slots = self.slots.borrow_mut();
        let sources =
            node_mut(&mut slots, id).map_or(Vec::new(), |node| std::mem::take(&mut node.sources));
        for source in sources {
            if let Some(source) = node_mut(&mut slots, source) {
                source.subscribers.retain(|&subscriber| subscriber != id);
            }
        }
    }

    /// Runs `run`, `id` subscribing what it reads, owned by `id`'s owner.
    fn run_tracked<R>(&self, id: Id, run: &dyn Fn() -> R) -> R {
        self.unsubscribe(id);
        let owner = {
            let slots = self.slots.borrow();
            node(&slots, id).and_then(|node| node.owner)
        };
        let observer = self.observer.replace(Some(id));
        let outer = self.owner.replace(owner);
        let result = run();
        self.owner.set(outer);
        self.observer.set(observer);
        result
    }

    /// Works the memo `id` out again, and tells what reads it if its value changed.
    fn work_out(&self, id: Id) {
        let compute = {
            let mut slots = self.slots.borrow_mut();
            let Some(node) = node_mut(&mut slots, id) else {
                return;
            };
            node.stale = false;
            let Kind::Memo(compute) = &node.kind else {
                return;
            };
            Rc::clone(compute)
        };
        self.runs.set(self.runs.get() + 1);
        if self.run_tracked(id, &*compute) {
            self.changed(id);
        }
    }

    /// Tells what reads `id` that it changed: its memos are stale, its bindings wait to run.
    pub(crate) fn changed(&self, id: Id) {
        let mut slots = self.slots.borrow_mut();
        let subscribers = node(&slots, id).map_or(Vec::new(), |node| node.subscribers.clone());
        for subscriber in subscribers {
            let Some(node) = node_mut(&mut slots, subscriber) else {
                continue;
            };
            match node.kind {
                Kind::Memo(_) if !node.stale => {
                    node.stale = true;
                    self.stale.borrow_mut().push_back(subscriber);
                }
                Kind::Binding(_) if !node.queued => {
                    node.queued = true;
                    self.queue.borrow_mut().push_back(subscriber);
                }
                _ => {}
            }
        }
    }

    /// Runs what waits, the memos first, until nothing does.
    pub(crate) fn flush(&self) {
        loop {
            let memo = self.stale.borrow_mut().pop_front();
            if let Some(memo) = memo {
                let stale = node(&self.slots.borrow(), memo).is_some_and(|node| node.stale);
                if stale {
                    self.work_out(memo);
                }
                continue;
            }
            let binding = self.queue.borrow_mut().pop_front();
            let Some(binding) = binding else {
                break;
            };
            let run = {
                let mut slots = self.slots.borrow_mut();
                let Some(node) = node_mut(&mut slots, binding) else {
                    continue;
                };
                node.queued = false;
                let Kind::Binding(run) = &node.kind else {
                    continue;
                };
                Rc::clone(run)
            };
            self.runs.set(self.runs.get() + 1);
            self.run_tracked(binding, &*run);
        }
    }

    /// Whether memos or bindings wait to run.
    pub(crate) fn waits(&self) -> bool {
        !self.stale.borrow().is_empty() || !self.queue.borrow().is_empty()
    }

    /// Disposes `id` and what it owns: what they read forgets them, their cleanups run, and each
    /// read of them after panics.
    pub(crate) fn dispose(&self, id: Id) {
        let (owned, owner, cleanups) = {
            let mut slots = self.slots.borrow_mut();
            let Some(node) = node_mut(&mut slots, id) else {
                return;
            };
            (
                std::mem::take(&mut node.owned),
                node.owner,
                std::mem::take(&mut node.cleanups),
            )
        };
        for owned in owned {
            self.dispose(owned);
        }
        for cleanup in cleanups {
            cleanup();
        }
        self.unsubscribe(id);
        let mut slots = self.slots.borrow_mut();
        if let Some(owner) = owner
            && let Some(owner) = node_mut(&mut slots, owner)
        {
            owner.owned.retain(|&owned| owned != id);
        }
        let slot = &mut slots[id.index as usize];
        slot.node = None;
        slot.generation += 1;
        self.free.borrow_mut().push(id.index);
    }
}

impl Node {
    fn new(kind: Kind, value: Option<Value>, owner: Option<Id>) -> Self {
        Self {
            kind,
            value,
            subscribers: Vec::new(),
            sources: Vec::new(),
            owner,
            owned: Vec::new(),
            cleanups: Vec::new(),
            stale: false,
            queued: false,
        }
    }
}

fn node(slots: &[Slot], id: Id) -> Option<&Node> {
    let slot = slots.get(id.index as usize)?;
    (slot.generation == id.generation)
        .then_some(slot.node.as_ref())
        .flatten()
}

fn node_mut(slots: &mut [Slot], id: Id) -> Option<&mut Node> {
    let slot = slots.get_mut(id.index as usize)?;
    (slot.generation == id.generation)
        .then_some(slot.node.as_mut())
        .flatten()
}
