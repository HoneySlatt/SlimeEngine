//! The render graph (ADR-0041): the passes of the renderer's frame, each declaring the targets it
//! draws into and the targets it samples. The graph orders the passes by those targets, creates the
//! *transient* targets (those whose content lives for one frame, at the output's size or a fraction
//! of it), records the passes in that order, a few to a submission, and gives each pass's GPU time.
//!
//! The renderer declares a graph when the frame's shape changes (the output's size, whether a
//! shadow pass or the bloom runs), not every frame: ordering and creating targets happen then, and
//! a frame only walks the ordered passes.

use std::fmt;
use std::time::Duration;

use slime_gfx::{
    DepthPassDescriptor, DepthTarget, GfxError, Gpu, GpuTimer, LinearColor, RenderPass,
    RenderPassDescriptor, RenderTarget, TextureFormat,
};

use crate::renderer::Pass;

/// The most passes one submission takes: the frame's passes are recorded in order, this many to a
/// submission. Fewer submissions cost the CPU less (on an RX 6900 XT, about 35 µs each with its
/// timer), but on a GPU whose memory that the CPU can map is a 256 MiB window (no resizable BAR),
/// nearly filled by the desktop's programs, one submission of a whole bloomed frame spent up to
/// 8 ms in the kernel in one frame of ten, where submissions of up to six passes never did
/// (ADR-0041).
const PASSES_PER_SUBMISSION: usize = 4;
/// A target of a graph, render or depth target: what a pass samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TargetId(usize);

/// A render target of a graph. Only the graph makes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ColorId(TargetId);

/// A depth target of a graph. Only the graph makes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DepthId(TargetId);

impl From<ColorId> for TargetId {
    fn from(id: ColorId) -> Self {
        id.0
    }
}

impl From<DepthId> for TargetId {
    fn from(id: DepthId) -> Self {
        id.0
    }
}

/// A target that outlives the graph, given to each [`Graph::record`]: the output, a window's image
/// that changes every frame, or the shadow map, which the renderer keeps.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Imported<'a> {
    /// A render target.
    Color(&'a RenderTarget),
    /// A depth target.
    Depth(&'a DepthTarget),
}

/// What a pass draws into: one of `slime-gfx`'s two kinds of pass, so that a pass the GPU layer
/// cannot run cannot be declared.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Draws {
    /// A render target, and a depth target of its size or none ([`RenderPassDescriptor`]): both
    /// cleared, to opaque black and to the farthest depth, when `clear`, or both kept, so that the
    /// pass draws over what earlier passes left.
    Color {
        target: ColorId,
        depth: Option<DepthId>,
        clear: bool,
    },
    /// A depth target alone, cleared to the farthest depth ([`DepthPassDescriptor`]).
    Depth(DepthId),
}

/// One pass of a graph.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PassDeclaration {
    /// What to record: [`Graph::record`] hands it back.
    pub(crate) pass: Pass,
    pub(crate) draws: Draws,
    /// The targets that the pass's bind groups hold, which it samples. They must be every target
    /// its groups hold: the graph orders the pass after the passes that draw them, and cannot see
    /// inside a bind group.
    pub(crate) reads: [Option<TargetId>; 2],
}

impl PassDeclaration {
    /// The targets the pass draws into.
    fn writes(&self) -> impl Iterator<Item = TargetId> {
        let (first, second) = match self.draws {
            Draws::Color { target, depth, .. } => (target.0, depth.map(|depth| depth.0)),
            Draws::Depth(target) => (target.0, None),
        };
        std::iter::once(first).chain(second)
    }

    fn reads(&self) -> impl Iterator<Item = TargetId> {
        self.reads.into_iter().flatten()
    }

    /// Whether the pass clears what it draws into, rather than keeping it.
    fn clears(&self) -> bool {
        match self.draws {
            Draws::Color { clear, .. } => clear,
            Draws::Depth(_) => true,
        }
    }
}

/// Where a target comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    /// Created by the graph, of this format (`None` for a depth target), at the output's size
    /// divided by 2^shift, rounded to the nearest.
    Transient {
        format: Option<TextureFormat>,
        shift: u32,
    },
    /// Given to each `record`, at this index of its `imported` list.
    Imported(usize),
}

/// A transient target created on the GPU.
#[derive(Clone, Debug)]
enum Created {
    Color(RenderTarget),
    Depth(DepthTarget),
}

#[derive(Debug)]
struct Target {
    /// Names the target in refusals and, for a transient one, in capture tools.
    label: String,
    source: Source,
    /// The transient target, once created.
    created: Option<Created>,
}

/// Why a graph cannot be recorded: each is a bug in its declaration, and names the passes and
/// targets at fault.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GraphError {
    /// A pass samples a transient target that no pass draws into, or is the first to draw into one
    /// and keeps it: it would read what an earlier frame left.
    Unwritten { pass: Pass, target: String },
    /// A pass clears a target that an earlier pass drew into, which would be lost unread.
    ClearsDrawnTarget {
        pass: Pass,
        target: String,
        earlier: Pass,
    },
    /// A pass samples a target it draws into, which the GPU cannot do.
    SamplesItsTarget { pass: Pass, target: String },
    /// The passes depend on each other in a cycle; these are the passes left unordered.
    Cycle { passes: Vec<Pass> },
    /// No pass uses a transient target, which would cost memory for nothing.
    Unused { target: String },
    /// Two transient targets have one label: the graph of the next shape finds the targets it
    /// reuses by their labels, and would give both the same texture.
    DuplicateTarget { target: String },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unwritten { pass, target } => write!(
                f,
                "pass {pass:?} reads `{target}`, which no earlier pass draws into"
            ),
            Self::ClearsDrawnTarget {
                pass,
                target,
                earlier,
            } => write!(
                f,
                "pass {pass:?} clears `{target}`, which pass {earlier:?} drew into before it"
            ),
            Self::SamplesItsTarget { pass, target } => {
                write!(f, "pass {pass:?} samples `{target}`, which it draws into")
            }
            Self::Cycle { passes } => {
                write!(f, "passes {passes:?} depend on each other in a cycle")
            }
            Self::Unused { target } => write!(f, "no pass uses target `{target}`"),
            Self::DuplicateTarget { target } => {
                write!(f, "two transient targets are labelled `{target}`")
            }
        }
    }
}

/// The side of a target `shift` halvings below a side of `side` texels: `side` divided by 2^shift,
/// rounded to the nearest, halves up, so that each level is half the one before, rounded down or
/// up, and stays within half a texel of the full side (ADR-0040). `shift` is below 32.
pub(crate) fn scaled_side(side: u32, shift: u32) -> u32 {
    (side + ((1 << shift) >> 1)) >> shift
}

/// The order to record `passes` in, given the `targets` they name, or why there is none.
///
/// The passes that draw into one target run in the order they were added, the first clearing it
/// (a transient one) and the others keeping it; a pass that samples a target runs after every pass
/// that draws into it. Among the passes these rules leave free, the earliest added runs first
/// (Kahn's algorithm), so that the order depends on the declarations alone, and is the order of
/// addition whenever that order is valid.
///
/// # Errors
///
/// The first refusal met: two transient targets with one label, then the passes checked in the
/// order they were added, then a cycle, then a target no pass uses (see [`GraphError`]).
fn order(targets: &[Target], passes: &[PassDeclaration]) -> Result<Vec<usize>, GraphError> {
    let label = |target: TargetId| targets[target.0].label.clone();
    let transient = |target: TargetId| matches!(targets[target.0].source, Source::Transient { .. });
    for (index, target) in targets.iter().enumerate() {
        let named_before = targets[..index]
            .iter()
            .enumerate()
            .any(|(other, earlier)| earlier.label == target.label && transient(TargetId(other)));
        if named_before && transient(TargetId(index)) {
            return Err(GraphError::DuplicateTarget {
                target: target.label.clone(),
            });
        }
    }
    // For each target, the passes that draw into it, in the order they were added.
    let mut writers: Vec<Vec<usize>> = vec![Vec::new(); targets.len()];
    for (index, pass) in passes.iter().enumerate() {
        for target in pass.writes() {
            writers[target.0].push(index);
        }
    }
    // For each pass, the passes it runs after.
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); passes.len()];
    for (index, pass) in passes.iter().enumerate() {
        for target in pass.reads() {
            if pass.writes().any(|written| written == target) {
                return Err(GraphError::SamplesItsTarget {
                    pass: pass.pass,
                    target: label(target),
                });
            }
            if transient(target) && writers[target.0].is_empty() {
                return Err(GraphError::Unwritten {
                    pass: pass.pass,
                    target: label(target),
                });
            }
            after[index].extend(&writers[target.0]);
        }
        for target in pass.writes() {
            let position = writers[target.0]
                .iter()
                .position(|&writer| writer == index)
                .expect("listed above");
            match (position, pass.clears()) {
                (0, false) if transient(target) => {
                    return Err(GraphError::Unwritten {
                        pass: pass.pass,
                        target: label(target),
                    });
                }
                (0, _) => {}
                (_, true) => {
                    return Err(GraphError::ClearsDrawnTarget {
                        pass: pass.pass,
                        target: label(target),
                        earlier: passes[writers[target.0][position - 1]].pass,
                    });
                }
                (_, false) => after[index].push(writers[target.0][position - 1]),
            }
        }
    }

    let mut placed = vec![false; passes.len()];
    let mut ordered = Vec::with_capacity(passes.len());
    while ordered.len() < passes.len() {
        let next = (0..passes.len())
            .find(|&index| !placed[index] && after[index].iter().all(|&before| placed[before]));
        let Some(next) = next else {
            return Err(GraphError::Cycle {
                passes: (0..passes.len())
                    .filter(|&index| !placed[index])
                    .map(|index| passes[index].pass)
                    .collect(),
            });
        };
        placed[next] = true;
        ordered.push(next);
    }

    for (index, target) in targets.iter().enumerate() {
        let used = !writers[index].is_empty()
            || passes
                .iter()
                .any(|pass| pass.reads().any(|read| read.0 == index));
        if !used && transient(TargetId(index)) {
            return Err(GraphError::Unused {
                target: target.label.clone(),
            });
        }
    }
    Ok(ordered)
}

/// The passes of a frame and their targets, for an output of one size.
#[derive(Debug)]
pub(crate) struct Graph {
    width: u32,
    height: u32,
    targets: Vec<Target>,
    /// In the order they were added, then, once compiled, in the order they run.
    passes: Vec<PassDeclaration>,
    imported: usize,
    compiled: bool,
    /// One per submission, timing its passes, created with the targets; none on adapters that
    /// cannot time their work.
    timers: Vec<GpuTimer>,
}

impl Graph {
    /// An empty graph for an output of `width` × `height` pixels.
    pub(crate) fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            targets: Vec::new(),
            passes: Vec::new(),
            imported: 0,
            compiled: false,
            timers: Vec::new(),
        }
    }

    /// Declares a transient render target of `format`, the output's size divided by 2^`shift`,
    /// named `label` in error messages and capture tools.
    ///
    /// # Panics
    ///
    /// If that size has a side of zero.
    pub(crate) fn transient_color(
        &mut self,
        label: impl Into<String>,
        format: TextureFormat,
        shift: u32,
    ) -> ColorId {
        ColorId(self.transient(label.into(), Some(format), shift))
    }

    /// Declares a transient depth target, the output's size divided by 2^`shift`, named `label`
    /// in error messages and capture tools.
    ///
    /// # Panics
    ///
    /// If that size has a side of zero.
    pub(crate) fn transient_depth(&mut self, label: impl Into<String>, shift: u32) -> DepthId {
        DepthId(self.transient(label.into(), None, shift))
    }

    fn transient(&mut self, label: String, format: Option<TextureFormat>, shift: u32) -> TargetId {
        assert!(
            scaled_side(self.width, shift) > 0 && scaled_side(self.height, shift) > 0,
            "transient target `{label}` would be empty: {}x{} divided by 2^{shift}",
            self.width,
            self.height
        );
        self.add_target(label, Source::Transient { format, shift })
    }

    /// Declares a render target that each [`record`](Self::record) is given, at the next index of
    /// its `imported` list. It holds what it held before the frame: a pass may keep or sample it
    /// without an earlier pass drawing into it.
    pub(crate) fn import_color(&mut self, label: impl Into<String>) -> ColorId {
        let source = Source::Imported(self.imported);
        self.imported += 1;
        ColorId(self.add_target(label.into(), source))
    }

    /// Declares a depth target that each [`record`](Self::record) is given, as
    /// [`import_color`](Self::import_color) does.
    pub(crate) fn import_depth(&mut self, label: impl Into<String>) -> DepthId {
        let source = Source::Imported(self.imported);
        self.imported += 1;
        DepthId(self.add_target(label.into(), source))
    }

    fn add_target(&mut self, label: String, source: Source) -> TargetId {
        self.targets.push(Target {
            label,
            source,
            created: None,
        });
        TargetId(self.targets.len() - 1)
    }

    /// Adds a pass. Passes run in the order they are added unless the targets they draw and
    /// sample ask for another (see [`compile`](Self::compile)).
    ///
    /// # Panics
    ///
    /// If the graph is compiled.
    pub(crate) fn add_pass(&mut self, pass: PassDeclaration) {
        assert!(!self.compiled, "pass {:?} added after compiling", pass.pass);
        self.passes.push(pass);
    }

    /// Orders the passes: the passes that draw into one target run in the order they were added,
    /// the first clearing it and the others keeping it, and a pass runs after every pass that
    /// draws a target it samples. Among the passes these rules leave free, the earliest added runs
    /// first, so that the order is deterministic and a frame declared in a valid order keeps it.
    ///
    /// # Errors
    ///
    /// A frame that cannot be drawn ([`GraphError`]): two transient targets with one label; a pass
    /// that samples a transient target no pass draws into, or is the first to draw into one and
    /// keeps it; a pass that clears a target an earlier pass drew into; a pass that samples what it
    /// draws into; passes that depend on each other in a cycle; a transient target that no pass
    /// uses. The graph is then left as it was.
    pub(crate) fn compile(&mut self) -> Result<(), GraphError> {
        let ordered = order(&self.targets, &self.passes)?;
        self.passes = ordered
            .into_iter()
            .map(|index| self.passes[index])
            .collect();
        self.compiled = true;
        Ok(())
    }

    /// Creates the transient targets on the GPU, reusing those of `previous` that have the same
    /// label, format and size, so that a frame whose shape changes but not its size keeps its
    /// high-dynamic-range target; then the timers of the passes, one per submission, on adapters
    /// that can time their work.
    ///
    /// # Errors
    ///
    /// The first error of a creation, as [`Gpu::create_render_target`] gives them: the GPU's
    /// memory cannot hold a target or a timer, or the device is lost. The graph is then dropped:
    /// its renderer keeps the frame it had.
    pub(crate) fn create(&mut self, gpu: &Gpu, previous: Option<&Graph>) -> Result<(), GfxError> {
        for index in 0..self.targets.len() {
            let Source::Transient { format, shift } = self.targets[index].source else {
                continue;
            };
            let size = self.size(shift);
            let target = &self.targets[index];
            let reused = previous.and_then(|previous| {
                previous.targets.iter().find_map(|old| match old.source {
                    Source::Transient {
                        format: old_format,
                        shift: old_shift,
                    } if old.label == target.label
                        && old_format == format
                        && previous.size(old_shift) == size =>
                    {
                        old.created.clone()
                    }
                    _ => None,
                })
            });
            let created = match (reused, format) {
                (Some(reused), _) => reused,
                (None, Some(format)) => Created::Color(gpu.create_render_target(
                    &target.label,
                    size.0,
                    size.1,
                    format,
                )?),
                (None, None) => {
                    Created::Depth(gpu.create_depth_target(&target.label, size.0, size.1)?)
                }
            };
            self.targets[index].created = Some(created);
        }
        let passes = self.passes.len();
        self.timers.clear();
        for (submission, first) in (0..passes).step_by(PASSES_PER_SUBMISSION).enumerate() {
            let timed = (passes - first).min(PASSES_PER_SUBMISSION);
            let timer = gpu.create_timer(
                &format!("frame {submission}"),
                u32::try_from(timed).expect("at most PASSES_PER_SUBMISSION"),
            )?;
            // An adapter that cannot time gives no timer to any submission.
            let Some(timer) = timer else { break };
            self.timers.push(timer);
        }
        Ok(())
    }

    /// The size of a transient target `shift` halvings below the output's.
    fn size(&self, shift: u32) -> (u32, u32) {
        (
            scaled_side(self.width, shift),
            scaled_side(self.height, shift),
        )
    }

    /// The transient render target `target`, for the bind groups that sample it.
    ///
    /// # Panics
    ///
    /// If `target` is imported or a depth target, or the targets are not created yet.
    pub(crate) fn color(&self, target: impl Into<TargetId>) -> &RenderTarget {
        let target = &self.targets[target.into().0];
        match &target.created {
            Some(Created::Color(created)) => created,
            _ => panic!("`{}` is not a created render target", target.label),
        }
    }

    /// The passes' declarations, in the order they run once compiled.
    pub(crate) fn declarations(&self) -> &[PassDeclaration] {
        &self.passes
    }

    /// Records the passes in order, [`PASSES_PER_SUBMISSION`] to a submission named `label`: for
    /// each, its targets (`imported` gives the imported ones, in the order they were declared)
    /// cleared or kept, timed by its submission's timer, then `record` with its index and the pass
    /// to draw.
    ///
    /// # Panics
    ///
    /// If the graph is not compiled or its targets not created, or if `imported` does not hold a
    /// target of the declared kind at each index.
    pub(crate) fn record(
        &self,
        gpu: &Gpu,
        label: &str,
        imported: &[Imported<'_>],
        mut record: impl FnMut(usize, Pass, &mut RenderPass<'_>),
    ) {
        assert!(self.compiled, "the graph is recorded before it is compiled");
        let color = |target: ColorId| -> &RenderTarget {
            let target = &self.targets[(target.0).0];
            match (target.source, &target.created) {
                (_, Some(Created::Color(created))) => created,
                (Source::Imported(index), _) => match imported[index] {
                    Imported::Color(target) => target,
                    Imported::Depth(_) => panic!("imported target {index} is not a render target"),
                },
                _ => panic!("`{}` is not a created render target", target.label),
            }
        };
        let depth = |target: DepthId| -> &DepthTarget {
            let target = &self.targets[(target.0).0];
            match (target.source, &target.created) {
                (_, Some(Created::Depth(created))) => created,
                (Source::Imported(index), _) => match imported[index] {
                    Imported::Depth(target) => target,
                    Imported::Color(_) => panic!("imported target {index} is not a depth target"),
                },
                _ => panic!("`{}` is not a created depth target", target.label),
            }
        };
        for (submission, passes) in self.passes.chunks(PASSES_PER_SUBMISSION).enumerate() {
            let timer = self.timers.get(submission);
            gpu.submit(label, |commands| {
                for (offset, pass) in passes.iter().enumerate() {
                    let index = submission * PASSES_PER_SUBMISSION + offset;
                    let draw = |render_pass: &mut RenderPass<'_>| {
                        record(index, pass.pass, render_pass);
                    };
                    match pass.draws {
                        Draws::Color {
                            target,
                            depth: depth_target,
                            clear,
                        } => commands.render(
                            &RenderPassDescriptor {
                                target: color(target),
                                clear: clear.then_some(LinearColor::BLACK),
                                depth: depth_target.map(depth),
                                timer,
                                resolve: None,
                            },
                            draw,
                        ),
                        Draws::Depth(target) => commands.render_depth(
                            &DepthPassDescriptor {
                                depth: depth(target),
                                timer,
                            },
                            draw,
                        ),
                    }
                }
            });
        }
    }

    /// The GPU time of the pass at `index` in the order the passes run, from the latest of its
    /// submissions whose times have come back: `None` until one has, and on adapters that cannot
    /// time their work.
    ///
    /// # Panics
    ///
    /// If `index` is not below the number of passes.
    pub(crate) fn gpu_time(&self, index: usize) -> Option<Duration> {
        assert!(
            index < self.passes.len(),
            "pass {index} of a graph of {} passes",
            self.passes.len()
        );
        self.timers
            .get(index / PASSES_PER_SUBMISSION)?
            .latest(index % PASSES_PER_SUBMISSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR: TextureFormat = TextureFormat::Rgba16Float;

    fn color(target: ColorId, clear: bool) -> Draws {
        Draws::Color {
            target,
            depth: None,
            clear,
        }
    }

    fn pass(pass: Pass, draws: Draws, reads: &[TargetId]) -> PassDeclaration {
        let mut slots = [None; 2];
        for (slot, &read) in slots.iter_mut().zip(reads) {
            *slot = Some(read);
        }
        PassDeclaration {
            pass,
            draws,
            reads: slots,
        }
    }

    fn order_of(graph: &mut Graph) -> Vec<Pass> {
        graph.compile().expect("the graph is valid");
        graph.declarations().iter().map(|pass| pass.pass).collect()
    }

    /// The renderer's frame with a shadow pass and a bloom of two levels, declared backwards.
    #[test]
    fn passes_run_after_the_passes_that_draw_what_they_read() {
        let mut graph = Graph::new(64, 32);
        let output = graph.import_color("output");
        let shadow_map = graph.import_depth("shadow map");
        let hdr = graph.transient_color("scene", HDR, 0);
        let depth = graph.transient_depth("scene depth", 0);
        let down = [
            graph.transient_color("down 0", HDR, 1),
            graph.transient_color("down 1", HDR, 2),
        ];
        let up = graph.transient_color("up 0", HDR, 1);
        graph.add_pass(pass(
            Pass::ToneMapping,
            color(output, true),
            &[hdr.into(), up.into()],
        ));
        graph.add_pass(pass(
            Pass::BloomUp(0),
            color(up, true),
            &[down[1].into(), down[0].into()],
        ));
        graph.add_pass(pass(
            Pass::BloomDown(1),
            color(down[1], true),
            &[down[0].into()],
        ));
        graph.add_pass(pass(
            Pass::BloomDown(0),
            color(down[0], true),
            &[hdr.into()],
        ));
        let scene = |clear| Draws::Color {
            target: hdr,
            depth: Some(depth),
            clear,
        };
        graph.add_pass(pass(Pass::Scene, scene(true), &[shadow_map.into()]));
        graph.add_pass(pass(Pass::Sky, scene(false), &[]));
        graph.add_pass(pass(Pass::Shadow, Draws::Depth(shadow_map), &[]));
        assert_eq!(
            order_of(&mut graph),
            [
                Pass::Shadow,
                Pass::Scene,
                Pass::Sky,
                Pass::BloomDown(0),
                Pass::BloomDown(1),
                Pass::BloomUp(0),
                Pass::ToneMapping
            ]
        );
    }

    #[test]
    fn passes_free_to_run_in_any_order_keep_the_order_they_were_added_in() {
        let mut graph = Graph::new(8, 8);
        let a = graph.transient_color("a", HDR, 0);
        let b = graph.transient_color("b", HDR, 0);
        let output = graph.import_color("output");
        graph.add_pass(pass(Pass::BloomDown(1), color(b, true), &[]));
        graph.add_pass(pass(Pass::BloomDown(0), color(a, true), &[]));
        graph.add_pass(pass(
            Pass::ToneMapping,
            color(output, true),
            &[a.into(), b.into()],
        ));
        assert_eq!(
            order_of(&mut graph),
            [Pass::BloomDown(1), Pass::BloomDown(0), Pass::ToneMapping]
        );
    }

    #[test]
    fn the_passes_drawing_into_a_target_follow_each_other_and_its_readers_follow_the_last() {
        // The sky draws over the scene, and the bloom reads what both drew, though it is added
        // between them.
        let mut graph = Graph::new(8, 8);
        let hdr = graph.transient_color("scene", HDR, 0);
        let down = graph.transient_color("down", HDR, 1);
        graph.add_pass(pass(Pass::Scene, color(hdr, true), &[]));
        graph.add_pass(pass(Pass::BloomDown(0), color(down, true), &[hdr.into()]));
        graph.add_pass(pass(Pass::Sky, color(hdr, false), &[]));
        assert_eq!(
            order_of(&mut graph),
            [Pass::Scene, Pass::Sky, Pass::BloomDown(0)]
        );
    }

    #[test]
    fn an_imported_target_can_be_read_and_kept_without_being_drawn_into() {
        // The shadow map of a frame without a shadow pass: the scene pass still binds it.
        let mut graph = Graph::new(8, 8);
        let shadow_map = graph.import_depth("shadow map");
        let hdr = graph.transient_color("scene", HDR, 0);
        let output = graph.import_color("output");
        graph.add_pass(pass(Pass::Scene, color(hdr, true), &[shadow_map.into()]));
        graph.add_pass(pass(Pass::ToneMapping, color(output, false), &[hdr.into()]));
        assert_eq!(order_of(&mut graph), [Pass::Scene, Pass::ToneMapping]);
    }

    #[test]
    fn a_read_of_a_transient_target_no_pass_draws_into_is_refused() {
        let mut graph = Graph::new(8, 8);
        let hdr = graph.transient_color("scene", HDR, 0);
        let output = graph.import_color("output");
        graph.add_pass(pass(Pass::ToneMapping, color(output, true), &[hdr.into()]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::Unwritten {
                pass: Pass::ToneMapping,
                target: "scene".to_owned()
            })
        );
    }

    #[test]
    fn keeping_a_transient_target_no_earlier_pass_drew_into_is_refused() {
        let mut graph = Graph::new(8, 8);
        let hdr = graph.transient_color("scene", HDR, 0);
        graph.add_pass(pass(Pass::Sky, color(hdr, false), &[]));
        graph.add_pass(pass(Pass::Scene, color(hdr, true), &[]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::Unwritten {
                pass: Pass::Sky,
                target: "scene".to_owned()
            })
        );
    }

    #[test]
    fn clearing_what_an_earlier_pass_drew_is_refused() {
        let mut graph = Graph::new(8, 8);
        let hdr = graph.transient_color("scene", HDR, 0);
        graph.add_pass(pass(Pass::Scene, color(hdr, true), &[]));
        graph.add_pass(pass(Pass::Sky, color(hdr, true), &[]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::ClearsDrawnTarget {
                pass: Pass::Sky,
                target: "scene".to_owned(),
                earlier: Pass::Scene
            })
        );
    }

    #[test]
    fn a_pass_sampling_what_it_draws_into_is_refused() {
        let mut graph = Graph::new(8, 8);
        let up = graph.transient_color("up", HDR, 0);
        graph.add_pass(pass(Pass::BloomUp(0), color(up, true), &[up.into()]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::SamplesItsTarget {
                pass: Pass::BloomUp(0),
                target: "up".to_owned()
            })
        );
        let mut graph = Graph::new(8, 8);
        let depth = graph.transient_depth("depth", 0);
        graph.add_pass(pass(Pass::Shadow, Draws::Depth(depth), &[depth.into()]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::SamplesItsTarget {
                pass: Pass::Shadow,
                target: "depth".to_owned()
            })
        );
    }

    #[test]
    fn a_cycle_is_refused_with_the_passes_in_it() {
        let mut graph = Graph::new(8, 8);
        let a = graph.transient_color("a", HDR, 0);
        let b = graph.transient_color("b", HDR, 0);
        let output = graph.import_color("output");
        graph.add_pass(pass(Pass::ToneMapping, color(output, true), &[]));
        graph.add_pass(pass(Pass::BloomDown(0), color(a, true), &[b.into()]));
        graph.add_pass(pass(Pass::BloomDown(1), color(b, true), &[a.into()]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::Cycle {
                passes: vec![Pass::BloomDown(0), Pass::BloomDown(1)]
            })
        );
    }

    #[test]
    fn a_transient_target_no_pass_uses_is_refused() {
        let mut graph = Graph::new(8, 8);
        let hdr = graph.transient_color("scene", HDR, 0);
        graph.transient_color("bloom down 9", HDR, 1);
        graph.import_depth("shadow map");
        graph.add_pass(pass(Pass::Scene, color(hdr, true), &[]));
        assert_eq!(
            graph.compile(),
            Err(GraphError::Unused {
                target: "bloom down 9".to_owned()
            })
        );
    }

    #[test]
    fn transient_targets_are_the_output_divided_by_a_power_of_two_rounded_to_the_nearest() {
        let graph = Graph::new(1921, 1081);
        assert_eq!(graph.size(0), (1921, 1081));
        assert_eq!(graph.size(1), (961, 541));
        // 1921 / 64 = 30.02 and 1081 / 64 = 16.9.
        assert_eq!(graph.size(6), (30, 17));
        // Halves round up: 3 / 2 is 2, and 1 halved is 1, quartered 0.
        assert_eq!(
            (scaled_side(3, 1), scaled_side(1, 1), scaled_side(1, 2)),
            (2, 1, 0)
        );
        for side in [1, 2, 7, 8, 15, 1080, 1081, 8192] {
            for shift in 0..14 {
                let exact = f64::from(side) / f64::from(1u32 << shift);
                let scaled = f64::from(scaled_side(side, shift));
                assert!(
                    (scaled - exact).abs() <= 0.5,
                    "{side} divided by 2^{shift} gives {scaled}, not within half of {exact}"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "transient target `tiny` would be empty: 3x2 divided by 2^3")]
    fn an_empty_transient_target_is_refused() {
        Graph::new(3, 2).transient_color("tiny", HDR, 3);
    }

    #[test]
    fn two_transient_targets_with_one_label_are_refused() {
        let mut graph = Graph::new(8, 8);
        let first = graph.transient_color("level", HDR, 1);
        let second = graph.transient_color("level", HDR, 2);
        graph.add_pass(pass(Pass::BloomDown(0), color(first, true), &[]));
        graph.add_pass(pass(
            Pass::BloomDown(1),
            color(second, true),
            &[first.into()],
        ));
        assert_eq!(
            graph.compile(),
            Err(GraphError::DuplicateTarget {
                target: "level".to_owned()
            })
        );
    }

    /// The graph of a new shape keeps the targets of the previous one that have its label, format
    /// and size, and creates the others: a kept target still holds what the previous graph's frame
    /// cleared it to, opaque black, where a new one holds zeros.
    #[test]
    fn the_next_shape_reuses_the_targets_of_the_same_label_format_and_size() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let declare = |width, height, format| {
            let mut graph = Graph::new(width, height);
            let output = graph.import_color("output");
            let kept = graph.transient_color("kept", HDR, 0);
            let other = graph.transient_color("other", format, 0);
            graph.add_pass(pass(Pass::BloomDown(0), color(kept, true), &[]));
            graph.add_pass(pass(Pass::BloomDown(1), color(other, true), &[]));
            graph.add_pass(pass(
                Pass::ToneMapping,
                color(output, true),
                &[kept.into(), other.into()],
            ));
            graph.compile().expect("the graph is valid");
            (graph, kept, other)
        };
        let alpha = |target: &RenderTarget| {
            if target.format() == HDR {
                let image = gpu.read_float_render_target(target).expect("read back");
                image.pixels[3]
            } else {
                let image = gpu.read_render_target(target).expect("read back");
                f32::from(image.pixels[3]) / 255.0
            }
        };

        let (mut first, kept, other) = declare(8, 8, HDR);
        first.create(&gpu, None).expect("the GPU has memory for it");
        let output = gpu
            .create_render_target("output", 8, 8, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        first.record(&gpu, "frame", &[Imported::Color(&output)], |_, _, _| {});
        assert_eq!(
            (alpha(first.color(kept)), alpha(first.color(other))),
            (1.0, 1.0)
        );

        // The same size, "other" in another format.
        let (mut second, kept, other) = declare(8, 8, TextureFormat::Rgba8Unorm);
        second
            .create(&gpu, Some(&first))
            .expect("the GPU has memory for it");
        assert_eq!(
            alpha(second.color(kept)),
            1.0,
            "the same label, format and size: kept"
        );
        assert_eq!(
            alpha(second.color(other)),
            0.0,
            "another format: created again"
        );

        // Another size.
        let (mut third, kept, _) = declare(8, 4, HDR);
        third
            .create(&gpu, Some(&second))
            .expect("the GPU has memory for it");
        let created = third.color(kept);
        assert_eq!((created.width(), created.height()), (8, 4));
        assert_eq!(alpha(created), 0.0, "another size: created again");
    }

    /// Records a frame of six passes, two submissions, the reader added before the pass that draws
    /// what it reads.
    #[test]
    fn the_graph_records_its_passes_in_order_into_targets_of_their_size_each_timed() {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let mut graph = Graph::new(7, 5);
        let output = graph.import_color("output");
        let half = graph.transient_color("half", HDR, 1);
        let levels: Vec<ColorId> = (0..4)
            .map(|index| graph.transient_color(format!("level {index}"), HDR, 0))
            .collect();
        graph.add_pass(pass(Pass::ToneMapping, color(output, true), &[half.into()]));
        graph.add_pass(pass(Pass::BloomDown(0), color(half, true), &[]));
        for (index, &level) in levels.iter().enumerate() {
            graph.add_pass(pass(Pass::BloomUp(index), color(level, true), &[]));
        }
        graph.compile().expect("the graph is valid");
        graph.create(&gpu, None).expect("the GPU has memory for it");
        let created = graph.color(half);
        assert_eq!((created.width(), created.height()), (4, 3), "7x5 halved");

        let target = gpu
            .create_render_target("output", 7, 5, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let mut times = vec![None; 6];
        for _frame in 0..8 {
            let mut recorded = Vec::new();
            graph.record(
                &gpu,
                "frame",
                &[Imported::Color(&target)],
                |index, pass, _render_pass| recorded.push((index, pass)),
            );
            assert_eq!(
                recorded,
                [
                    (0, Pass::BloomDown(0)),
                    (1, Pass::ToneMapping),
                    (2, Pass::BloomUp(0)),
                    (3, Pass::BloomUp(1)),
                    (4, Pass::BloomUp(2)),
                    (5, Pass::BloomUp(3)),
                ]
            );
            // Waiting is not needed to get the times back, but makes the test deterministic.
            gpu.wait_idle().expect("the GPU finishes the passes");
            times = (0..6).map(|index| graph.gpu_time(index)).collect();
            if !gpu.can_time() || times.iter().all(Option::is_some) {
                break;
            }
        }
        if gpu.can_time() {
            assert!(
                times.iter().all(Option::is_some),
                "each pass gets its time back from its submission's timer: {times:?}"
            );
        } else {
            assert!(times.iter().all(Option::is_none));
        }
    }
}
