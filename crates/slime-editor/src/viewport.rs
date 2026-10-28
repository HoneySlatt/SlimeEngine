//! The viewports: the level drawn by the editor's own asset cache and frame builder, which the
//! viewports share ([`Level`]), into an image of each that a surface of the window shows, drawn
//! again only when what it shows changed (`docs/adr/0158-the-viewport.md`), the ones not navigated
//! resting (`docs/adr/0160-two-viewports-and-blenders-way.md`). Their cameras move as
//! [`Navigation`](crate::navigation) says; the grid on the ground and the icons of the lights are
//! drawn into it, and what is selected outlined, an entity that draws nothing as its unit box, its
//! meshes' wireframe too when asked; the window asks it what is drawn where
//! (`docs/adr/0159-picking-and-the-selection.md`).

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use slime_assets::{AssetId, CookedAssets};
use slime_color::LinearColor;
use slime_gfx::{AddressMode, Filter, Gpu, RenderTarget, SamplerDescriptor, TextureFormat};
use slime_reflect::Reflect;
use slime_render::glam::{DAffine3, DVec3, Vec2, Vec3};
use slime_render::{
    AlphaMode, AssetCache, Camera, DrawList, Drawings, Instance, Light, Line, Material, Mesh,
    MeshData, Outline, Pick, RenderError, RenderStats, Renderer, SampledTexture, Scene, UnlitData,
    View, ViewDraw, ViewSettings, facing,
};
use slime_ui::display::ImageKey;
use slime_ui::look;
use slime_ui::text::{Fonts, GlyphPixels, GlyphRasterizer, TextBounds, TextStyle};
use slime_world::{Entity, Meshes, Tick, Transform, World};
use slime_world_render::{FrameBuilder, Picker};

use crate::program::Lighting;

/// The key of the image of the viewport of the window's pane `pane`, from 0, in the window's
/// display list, which its surface shows.
pub(crate) fn image_key(pane: usize) -> ImageKey {
    ImageKey(1 + pane as u64)
}

/// The name by which tools capture the viewport of the pane `pane`, from 0, change its settings and
/// read its report: `viewport`, then `viewport 2`.
pub(crate) fn name(pane: usize) -> String {
    match pane {
        0 => "viewport".to_owned(),
        pane => format!("viewport {}", pane + 1),
    }
}

/// The grid on the ground: its height, a hair above the ground so that the ground does not hide
/// it, how far it reaches from the point that the camera looks at, its spacing, color and width.
const GRID_HEIGHT: f64 = 0.005;
const GRID_REACH: f64 = 50.0;
const GRID_SPACING: f64 = 1.0;
const GRID_COLOR: LinearColor = LinearColor::rgba(0.5, 0.5, 0.5, 0.35);
const GRID_WIDTH: f32 = 1.0;

/// The side of a light's icon in the scene, in metres, and of its image, in texels.
const ICON_SIDE: f64 = 0.5;
const ICON_TEXELS: u32 = 64;

/// The size of the Lucide glyph drawn on an icon, in texels of its image.
const ICON_GLYPH: f32 = 40.0;

/// The outline of what is selected, in the colors of `slime-render`'s by default, its width in
/// logical pixels; the box of a selected entity that draws nothing, the outline's color, and its
/// width; the wireframe of what is selected, its color and width.
const OUTLINE_WIDTH: f32 = 2.0;
const BOX_WIDTH: f32 = 1.5;
const WIREFRAME_COLOR: LinearColor = LinearColor::rgba(1.0, 0.8, 0.55, 0.5);
const WIREFRAME_WIDTH: f32 = 1.0;

/// How often a viewport draws at most while another is navigated, drawn at every frame: one
/// viewport at the full rate, the others at a tenth of a second's
/// (`docs/adr/0160-two-viewports-and-blenders-way.md`).
const RESTING: std::time::Duration = std::time::Duration::from_millis(100);

/// What the image shows: when it differs, the image is drawn again.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    tick: Tick,
    camera: Camera,
    size: (u32, u32),
    settings: ViewSettings,
    marks: Marks,
}

/// What the viewport marks of the level: what is selected, outlined, and whether its wireframe is
/// drawn, in widths of `scale` pixels of the screen per logical pixel.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Marks {
    pub(crate) selection: Vec<Entity>,
    pub(crate) wireframe: bool,
    pub(crate) scale: f32,
}

/// What the viewports share, on one GPU device: the level's meshes loaded from the cooked data, the
/// frame builder that draws the world from them, the icons of the lights, how the level is lit, the
/// picker that casts rays through it, and the edges that a wireframe read.
pub(crate) struct Level {
    cache: AssetCache,
    builder: FrameBuilder,
    icons: Icons,
    lighting: Lighting,
    /// The cooked data, which the wireframe reads meshes from.
    assets: CookedAssets,
    picker: Picker,
    /// The edges of each mesh that the wireframe drew, by their ends in the mesh's space.
    edges: HashMap<AssetId, Vec<(Vec3, Vec3)>>,
}

impl Level {
    /// The level's meshes loaded from `assets` by `renderer`, lit as `lighting` says.
    ///
    /// # Errors
    ///
    /// When the GPU cannot make the icons.
    pub(crate) fn new(
        gpu: &Gpu,
        renderer: &Renderer,
        assets: CookedAssets,
        lighting: Lighting,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            cache: AssetCache::new(renderer, assets.clone()),
            builder: FrameBuilder::new(lighting.sky),
            icons: Icons::new(gpu, renderer)?,
            lighting,
            picker: Picker::new(assets.clone()),
            assets,
            edges: HashMap::new(),
        })
    }

    /// Loads from `assets` from now on, the cooked data of a level opened since: what did not
    /// change is kept ([`AssetCache::reload`]).
    pub(crate) fn reload(&mut self, gpu: &Gpu, assets: CookedAssets) {
        self.cache.reload(gpu, assets.clone());
        self.picker = Picker::new(assets.clone());
        self.assets = assets;
        self.edges.clear();
    }

    /// The entities that the ray from `camera` through the pixel `at` of an image of `size`
    /// pixels meets, each once, the nearest first, by the CPU ([`Picker::cast_through`]).
    pub(crate) fn cast_through(
        &mut self,
        world: &World,
        camera: &Camera,
        size: (u32, u32),
        at: [f32; 2],
    ) -> Vec<Entity> {
        let (width, height) = (size.0 as f32, size.1 as f32);
        let point = Vec2::new(at[0] / width, at[1] / height);
        let ray = camera.ray(width / height, point);
        self.picker
            .cast_through(world, 1.0, ray)
            .into_iter()
            .map(|hit| hit.entity)
            .collect()
    }

    /// Marks in `drawings` what `marks` select of `world`, which `scene` draws: the instances of
    /// each entity selected and of those below it outlined, its light's icon too; a box of a metre
    /// around each one whose own and whose children's draw nothing, as its transform places it; and
    /// the edges of their meshes, when the wireframe is asked for.
    fn mark(&mut self, world: &World, scene: &Scene, drawings: &mut Drawings, marks: &Marks) {
        // The edges read for a wireframe, a terrain's many, are let go once it is off.
        if !marks.wireframe {
            self.edges.clear();
        }
        if marks.selection.is_empty() {
            return;
        }
        let selected: HashSet<Entity> = marks.selection.iter().copied().collect();
        // Whether `entity` is selected or lies below what is.
        let marked = |entity: Entity| {
            std::iter::successors(Some(entity), |&entity| world.parent(entity))
                .any(|each| selected.contains(&each))
        };
        let outline = Outline::default();
        let mut picks = HashSet::new();
        let mut drawing = HashSet::new();
        let instances = scene.instances.iter().chain(&drawings.in_scene.instances);
        for instance in instances {
            let Some(entity) = Entity::from_bits(instance.pick).filter(|&entity| marked(entity))
            else {
                continue;
            };
            picks.insert(instance.pick);
            drawing.extend(
                std::iter::successors(Some(entity), |&entity| world.parent(entity))
                    .filter(|each| selected.contains(each)),
            );
        }
        drawings.outline = Outline {
            picks,
            width: OUTLINE_WIDTH * marks.scale,
            ..outline
        };
        for &entity in &marks.selection {
            if drawing.contains(&entity) {
                continue;
            }
            let Some(place) = placed(world, entity) else {
                continue;
            };
            let corner = |corner: usize| {
                let sign = |bit: usize| if corner & bit == 0 { -0.5 } else { 0.5 };
                place.transform_point3(DVec3::new(sign(1), sign(2), sign(4)))
            };
            // The twelve edges: the corners that differ along one axis.
            for from in 0..8 {
                for axis in [1, 2, 4] {
                    if from & axis == 0 {
                        drawings.over_image.lines.push(Line {
                            from: corner(from),
                            to: corner(from | axis),
                            color: outline.seen,
                            width: BOX_WIDTH * marks.scale,
                        });
                    }
                }
            }
        }
        if marks.wireframe {
            self.wireframe(world, &marked, marks.scale, &mut drawings.over_image.lines);
        }
    }

    /// Adds to `lines` the edges of the meshes of each entity of `world` that `marked` says, as its
    /// transform places them, in widths of `scale` pixels per logical pixel; a mesh that cannot
    /// be read is left out.
    fn wireframe(
        &mut self,
        world: &World,
        marked: &dyn Fn(Entity) -> bool,
        scale: f32,
        lines: &mut Vec<Line>,
    ) {
        for (entity, meshes) in world.query::<&Meshes>() {
            if world.is_hidden(entity) || !marked(entity) {
                continue;
            }
            let Some(place) = placed(world, entity) else {
                continue;
            };
            for &id in &meshes.ids {
                let assets = &self.assets;
                let edges = self.edges.entry(id).or_insert_with(|| edges(assets, id));
                lines.extend(edges.iter().map(|&(from, to)| Line {
                    from: place.transform_point3(from.as_dvec3()),
                    to: place.transform_point3(to.as_dvec3()),
                    color: WIREFRAME_COLOR,
                    width: WIREFRAME_WIDTH * scale,
                }));
            }
        }
    }

    /// The sphere around what `world` draws of `entities` and the entities below them, or of the
    /// whole level when `entities` is empty: its center and radius, the box of each mesh, and the
    /// place of an entity that draws none, a metre around it; [`Framing::Loading`] while meshes
    /// that the world names still load, which the window asks again at a later frame.
    ///
    /// # Errors
    ///
    /// As [`FrameBuilder::build`].
    pub(crate) fn bounds(
        &mut self,
        gpu: &Gpu,
        world: &World,
        entities: &[Entity],
    ) -> Result<Framing, RenderError> {
        // Whether `entity` is `below` or lies below it.
        let under = |entity: Entity, below: Entity| {
            std::iter::successors(Some(entity), |&entity| world.parent(entity))
                .any(|each| each == below)
        };
        let within = |entity: Entity| {
            entities.is_empty() || entities.iter().any(|&selected| under(entity, selected))
        };
        let frame = self.builder.build(world, &mut self.cache, gpu, 1.0)?;
        if frame.loading > 0 {
            return Ok(Framing::Loading);
        }
        let mut boxes: Vec<(DVec3, DVec3)> = frame
            .instances
            .iter()
            .filter(|instance| Entity::from_bits(instance.pick).is_some_and(within))
            .map(|instance| {
                let (center, half) = instance.mesh.bounds();
                let (center, half) = (center.as_dvec3(), half.as_dvec3());
                // The box's eight corners, where the instance places them.
                let corners = (0..8).map(|corner| {
                    let sign = |bit: u32| if corner & bit == 0 { -1.0 } else { 1.0 };
                    let local = center + half * DVec3::new(sign(1), sign(2), sign(4));
                    instance.transform.transform_point3(local)
                });
                corners.fold((DVec3::INFINITY, DVec3::NEG_INFINITY), |(low, high), at| {
                    (low.min(at), high.max(at))
                })
            })
            .collect();
        // The selected entities that draw nothing, a metre around where they stand.
        for &entity in entities {
            let draws = frame.instances.iter().any(|instance| {
                Entity::from_bits(instance.pick).is_some_and(|drawn| under(drawn, entity))
            });
            if !draws && let Some(at) = world.interpolated_transform(entity, 1.0) {
                boxes.push((at.translation - DVec3::ONE, at.translation + DVec3::ONE));
            }
        }
        let joined = boxes
            .into_iter()
            .reduce(|(low, high), (other_low, other_high)| {
                (low.min(other_low), high.max(other_high))
            });
        Ok(joined.map_or(Framing::Nothing, |(low, high)| {
            Framing::Sphere((low + high) / 2.0, (high - low).length() / 2.0)
        }))
    }
}

/// A viewport's view and its image, which the level shared by every viewport is drawn into.
pub(crate) struct Viewport {
    view: View,
    format: TextureFormat,
    /// The image that the surface shows, at the surface's size in pixels.
    image: Option<RenderTarget>,
    /// What the image shows, once drawn.
    shown: Option<Shown>,
    /// Whether the last frame built waited for meshes still loading.
    loading: bool,
    /// What the next [`Viewport::draw`] draws.
    pending: Option<(Scene, Camera, Drawings)>,
    settings: ViewSettings,
    /// How many times it drew its image: a test of drawing on demand counts them.
    draws: u64,
    /// Its name, which names its image too.
    name: String,
    /// When it last made ready what it draws, which it does at most every [`RESTING`] while
    /// another viewport is navigated.
    drawn_at: Option<Instant>,
    /// How many pickings wait for a frame that draws them: the view draws one at each frame, the
    /// oldest first ([`View::pick`]), and the next ones draw the image whatever it shows until
    /// each has been.
    forced: usize,
}

impl Viewport {
    /// A viewport named `name` ([`name`]) that `renderer` draws, in its format.
    ///
    /// # Errors
    ///
    /// When the GPU cannot make the view.
    pub(crate) fn new(
        gpu: &Gpu,
        renderer: &Renderer,
        name: &str,
        format: TextureFormat,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            view: renderer.create_view(gpu, name)?,
            name: name.to_owned(),
            drawn_at: None,
            forced: 0,
            format,
            image: None,
            shown: None,
            loading: false,
            pending: None,
            settings: ViewSettings::default(),
            draws: 0,
        })
    }

    /// The image that the surface shows, once made.
    pub(crate) fn image(&self) -> Option<&RenderTarget> {
        self.image.as_ref()
    }

    /// Draws it as `settings` say from the next frame.
    pub(crate) fn set_settings(&mut self, settings: ViewSettings) {
        self.settings = settings;
    }

    /// Draws its image again at the next frame, as after the level's cooked data was read again.
    pub(crate) fn redraw(&mut self) {
        self.shown = None;
    }

    /// Asks what is drawn in `area` of its image, in its pixels, which the next frame draws
    /// ([`View::pick`]): its answer comes a frame or two later.
    pub(crate) fn pick(&mut self, area: slime_render::Viewport) -> Pick {
        self.forced += 1;
        self.view.pick(area)
    }

    /// What it reports to tools: how many times it drew its image, and what the last took.
    pub(crate) fn report(&self) -> Report {
        Report {
            draws: self.draws,
            stats: self.view.stats(),
        }
    }

    /// Makes ready what the next [`Viewport::draw`] draws of `world`, through `level`, from
    /// `camera`, at `size` in pixels, at `now`, marking it as `marks` say, if what the image shows
    /// changed: the world, the camera, the size, the settings or the marks, a mesh that came since
    /// the last frame, or a new size that has held still ([`View::resize_due`]); or if a picking
    /// waits for a frame. While another viewport is navigated, as `resting` says, it does so at
    /// most every [`RESTING`], but for an image just made, which it draws at once. Gives when the
    /// window is to ask for a frame: at once while meshes still load, or as its rest ends.
    ///
    /// # Errors
    ///
    /// When the GPU cannot make the image, or the frame cannot be built
    /// ([`FrameBuilder::build`]).
    #[expect(
        clippy::too_many_arguments,
        reason = "what a frame of the window gives each viewport, gathered by the window"
    )]
    pub(crate) fn prepare(
        &mut self,
        level: &mut Level,
        gpu: &Gpu,
        world: &World,
        camera: Camera,
        size: (u32, u32),
        marks: &Marks,
        resting: bool,
        now: Instant,
    ) -> Result<Option<Instant>, RenderError> {
        if self
            .image
            .as_ref()
            .is_none_or(|image| (image.width(), image.height()) != size)
        {
            self.image = Some(gpu.create_render_target(&self.name, size.0, size.1, self.format)?);
            self.shown = None;
        }
        let shown = Shown {
            tick: world.tick(),
            camera,
            size,
            settings: self.settings,
            marks: marks.clone(),
        };
        let settled = self.view.resize_due().is_some_and(|due| due <= now);
        if self.shown.as_ref() == Some(&shown) && !self.loading && !settled && self.forced == 0 {
            return Ok(None);
        }
        // A picking, and an image just made, are drawn at once, whatever the rest.
        if let Some(rested) = self
            .drawn_at
            .map(|drawn| drawn + RESTING)
            .filter(|&rested| resting && self.forced == 0 && self.shown.is_some() && now < rested)
        {
            return Ok(Some(rested));
        }
        self.drawn_at = Some(now);
        self.forced = self.forced.saturating_sub(1);
        let frame = level.builder.build(world, &mut level.cache, gpu, 1.0)?;
        self.loading = frame.loading > 0;
        let icons = frame
            .lights
            .iter()
            .filter_map(|&(entity, light)| {
                let (position, material) = match light {
                    Light::Point(light) => (light.position, &level.icons.point),
                    Light::Spot(light) => (light.position, &level.icons.spot),
                    // A directional light shows where its entity stands, the sky's sun nowhere.
                    Light::Directional(_) => (
                        world.interpolated_transform(entity, 1.0)?.translation,
                        &level.icons.directional,
                    ),
                };
                // Above the light, out of the lamp that holds it.
                let above = position + DVec3::Y * ICON_SIDE;
                let mut icon = Instance::new(
                    &level.icons.square,
                    material,
                    facing(above, camera.eye, ICON_SIDE),
                );
                icon.casts_shadows = false;
                // Picked as its light, which a click selects.
                icon.pick = entity.to_bits();
                Some(icon)
            })
            .collect();
        let mut drawings = Drawings {
            in_scene: DrawList {
                lines: Line::grid(
                    camera.target,
                    GRID_HEIGHT,
                    GRID_REACH,
                    GRID_SPACING,
                    GRID_COLOR,
                    GRID_WIDTH,
                ),
                instances: icons,
            },
            ..Drawings::default()
        };
        let mut scene = frame.scene(camera.eye);
        level.mark(world, &scene, &mut drawings, marks);
        scene.exposure = level.lighting.exposure;
        scene.bloom = level.lighting.bloom;
        self.pending = Some((scene, camera, drawings));
        self.shown = Some(shown);
        Ok(self.loading.then_some(now))
    }

    /// Draws what [`Viewport::prepare`] made ready into the image, if anything. Gives when a new
    /// size of the image that has not held still yet will have, at which the window asks for a
    /// frame ([`View::resize_due`]).
    ///
    /// # Errors
    ///
    /// When the renderer cannot draw it ([`Renderer::render`]).
    pub(crate) fn draw(
        &mut self,
        gpu: &Gpu,
        renderer: &mut Renderer,
    ) -> Result<Option<Instant>, RenderError> {
        let (Some((scene, camera, drawings)), Some(image)) = (self.pending.take(), &self.image)
        else {
            return Ok(self.view.resize_due());
        };
        self.view.set_settings(self.settings);
        let mut draw = ViewDraw::new(&mut self.view, camera, image);
        draw.drawings = Some(&drawings);
        renderer.render(gpu, &scene, &mut [draw])?;
        self.draws += 1;
        Ok(self.view.resize_due())
    }
}

/// Where `world` places `entity` and its meshes, as the frame builder draws them: its transform,
/// its stretch with it; `None` for an entity without a transform.
fn placed(world: &World, entity: Entity) -> Option<DAffine3> {
    let stretch = world.get::<Transform>(entity)?.stretch;
    Some(
        world
            .interpolated_transform(entity, 1.0)?
            .to_affine(stretch),
    )
}

/// The edges of the mesh `id` of `assets`, each once, by their ends; none, with a warning, if it
/// cannot be read.
fn edges(assets: &CookedAssets, id: AssetId) -> Vec<(Vec3, Vec3)> {
    match assets.mesh(id) {
        Ok(mesh) => edges_of(&mesh.positions, &mesh.indices),
        Err(error) => {
            tracing::warn!(%id, "a mesh is left out of the wireframe: {error}");
            Vec::new()
        }
    }
}

/// The edges of the triangles that `indices` make of `positions`, each once, whichever triangles
/// share it and whichever way they run along it, by their ends; an index beyond the positions
/// gives none.
fn edges_of(positions: &[[f32; 3]], indices: &[u32]) -> Vec<(Vec3, Vec3)> {
    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for &[a, b, c] in indices.as_chunks::<3>().0 {
        for (from, to) in [(a, b), (b, c), (c, a)] {
            if !seen.insert((from.min(to), from.max(to))) {
                continue;
            }
            if let (Some(&from), Some(&to)) =
                (positions.get(from as usize), positions.get(to as usize))
            {
                edges.push((Vec3::from(from), Vec3::from(to)));
            }
        }
    }
    edges
}

/// What the viewport reports to tools, as the report `viewport`: how many times it drew its image
/// since it was made, as the editor started or since the GPU device was last lost, which an editor
/// left alone does not change, and what its last drawing took.
#[derive(Clone, Copy, Debug, Reflect)]
#[reflect(name = "ViewportReport")]
pub(crate) struct Report {
    /// How many times it drew its image.
    pub(crate) draws: u64,
    /// What its last drawing took.
    pub(crate) stats: RenderStats,
}

/// What [`Viewport::bounds`] found to frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Framing {
    /// Meshes that the world names still load: the bounds are not known yet.
    Loading,
    /// What it was asked for draws nothing and stands nowhere.
    Nothing,
    /// The sphere around it: its center and radius, in metres.
    Sphere(DVec3, f64),
}

/// The icons of the lights: a square, and a material for each kind of light, the Lucide glyph of
/// its kind in white on a dark disc, unlit and blended.
struct Icons {
    square: Mesh,
    point: Material,
    spot: Material,
    directional: Material,
}

impl Icons {
    fn new(gpu: &Gpu, renderer: &Renderer) -> Result<Self, RenderError> {
        let mut fonts = Fonts::new();
        look::register_fonts(&mut fonts);
        let mut rasterizer = GlyphRasterizer::new();
        let sampler = gpu.create_sampler(
            "light icons",
            &SamplerDescriptor {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                address_u: AddressMode::ClampToEdge,
                address_v: AddressMode::ClampToEdge,
            },
        )?;
        let mut material = |name: &str| -> Result<Material, RenderError> {
            let texels = icon_texels(&mut fonts, &mut rasterizer, name);
            let texture = gpu.create_texture(
                name,
                ICON_TEXELS,
                ICON_TEXELS,
                TextureFormat::Rgba8UnormSrgb,
                &texels,
            )?;
            renderer.create_unlit_material(
                gpu,
                name,
                &UnlitData {
                    texture: Some(SampledTexture {
                        texture: &texture,
                        sampler: &sampler,
                    }),
                    alpha_mode: AlphaMode::Blend,
                    double_sided: true,
                    ..UnlitData::default()
                },
            )
        };
        Ok(Self {
            point: material("lightbulb")?,
            spot: material("flashlight")?,
            directional: material("sun")?,
            square: renderer.create_mesh(gpu, "light icon", &MeshData::square())?,
        })
    }
}

/// The texels of an icon, in sRGB with alpha: Lucide's glyph `name` in white, in the middle of a
/// dark disc whose edge fades over a texel.
fn icon_texels(fonts: &mut Fonts, rasterizer: &mut GlyphRasterizer, name: &str) -> Vec<u8> {
    let side = ICON_TEXELS as usize;
    let middle = ICON_TEXELS as f32 / 2.0;
    let mut texels: Vec<u8> = (0..side * side)
        .flat_map(|index| {
            let (x, y) = ((index % side) as f32, (index / side) as f32);
            let distance = (x + 0.5 - middle).hypot(y + 0.5 - middle);
            let alpha = (middle - 1.0 - distance).clamp(0.0, 1.0) * 0.75;
            [24, 26, 32, (alpha * 255.0).round() as u8]
        })
        .collect();
    let style = TextStyle {
        fonts: vec![look::ICON_FAMILY.to_owned()],
        size: ICON_GLYPH,
        ..TextStyle::default()
    };
    let text = look::icon(name).map(String::from).unwrap_or_default();
    let layout = fonts.lay_out(&text, &style, 1.0, TextBounds::default());
    let Some(glyph) = layout.runs().iter().flat_map(|run| run.placed()).next() else {
        return texels;
    };
    let Some(image) = rasterizer.rasterize(fonts, glyph.key) else {
        return texels;
    };
    let GlyphPixels::Coverage(coverage) = &image.pixels else {
        return texels;
    };
    // The glyph's image in the middle of the disc.
    let left = (ICON_TEXELS as i64 - i64::from(image.width)) / 2;
    let top = (ICON_TEXELS as i64 - i64::from(image.height)) / 2;
    for row in 0..image.height as i64 {
        for column in 0..image.width as i64 {
            let (x, y) = (left + column, top + row);
            if !(0..ICON_TEXELS as i64).contains(&x) || !(0..ICON_TEXELS as i64).contains(&y) {
                continue;
            }
            let cover =
                f32::from(coverage[(row * i64::from(image.width) + column) as usize]) / 255.0;
            let at = (y as usize * side + x as usize) * 4;
            // White over the disc, by the glyph's coverage.
            let under = f32::from(texels[at + 3]) / 255.0;
            let alpha = cover + under * (1.0 - cover);
            for channel in 0..3 {
                let below = f32::from(texels[at + channel]) * under * (1.0 - cover);
                let value = if alpha > 0.0 {
                    (255.0 * cover + below) / alpha
                } else {
                    0.0
                };
                texels[at + channel] = value.round().clamp(0.0, 255.0) as u8;
            }
            texels[at + 3] = (alpha * 255.0).round() as u8;
        }
    }
    texels
}

#[cfg(test)]
mod tests {
    use std::fs;

    use slime_render::glam::DVec3;
    use slime_world::Transform;

    use super::*;

    /// A level and a viewport of a renderer on the GPU that the tests run on, the level's cooked
    /// data empty, and the directory that holds it, removed by the caller.
    fn viewport(name: &str) -> (Gpu, Renderer, Level, Viewport, std::path::PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "slime-editor-viewport-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("a directory");
        fs::write(
            directory.join(slime_assets::MANIFEST_FILE),
            slime_assets::Manifest::new().to_bytes(),
        )
        .expect("an empty manifest");
        let assets = CookedAssets::open(&directory).expect("the cooked data");
        let gpu = Gpu::new().expect("a GPU adapter, as CONTRIBUTING.md says");
        let format = TextureFormat::Rgba8UnormSrgb;
        let renderer = Renderer::new(&gpu, format).expect("the renderer");
        let level = Level::new(&gpu, &renderer, assets, Lighting::default()).expect("the level");
        let viewport = Viewport::new(&gpu, &renderer, "viewport", format).expect("the viewport");
        (gpu, renderer, level, viewport, directory)
    }

    fn marks(selection: Vec<Entity>) -> Marks {
        Marks {
            selection,
            wireframe: false,
            scale: 1.0,
        }
    }

    #[test]
    fn each_picking_asked_draws_the_image_once_and_then_it_rests() {
        let (gpu, mut renderer, mut level, mut viewport, directory) = viewport("picks");
        let world = World::new();
        let camera = Camera::default();
        let size = (32, 24);
        let mut frame = |viewport: &mut Viewport| {
            viewport
                .prepare(
                    &mut level,
                    &gpu,
                    &world,
                    camera,
                    size,
                    &marks(Vec::new()),
                    false,
                    Instant::now(),
                )
                .expect("prepared");
            viewport.draw(&gpu, &mut renderer).expect("drawn");
            viewport.report().draws
        };
        assert_eq!(frame(&mut viewport), 1);
        assert_eq!(frame(&mut viewport), 1, "nothing changed");
        // Two pickings asked before a frame: the view draws one at each, the oldest first.
        let area = slime_render::Viewport {
            x: 4,
            y: 4,
            width: 1,
            height: 1,
        };
        let (first, second) = (viewport.pick(area), viewport.pick(area));
        assert_eq!(frame(&mut viewport), 2);
        assert_eq!(frame(&mut viewport), 3);
        assert_eq!(frame(&mut viewport), 3, "both drawn: it rests");
        let answered = |pick: &Pick| {
            (0..100).find_map(|_| {
                // As a frame's submissions would, which the window asks for meanwhile.
                gpu.wait_idle().expect("the device is there");
                pick.take(&gpu)
            })
        };
        for pick in [first, second] {
            let picked = answered(&pick).expect("answered").expect("drawn");
            assert_eq!(picked.at(0, 0), 0, "nothing in an empty world");
        }
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_viewport_that_rests_draws_at_most_once_in_its_rest() {
        let (gpu, mut renderer, mut level, mut viewport, directory) = viewport("rest");
        let world = World::new();
        let size = (32, 24);
        let start = Instant::now();
        let mut frame = |viewport: &mut Viewport, eye: f64, at: Instant| {
            let camera = Camera {
                eye: DVec3::new(eye, 0.0, 5.0),
                ..Camera::default()
            };
            let due = viewport
                .prepare(
                    &mut level,
                    &gpu,
                    &world,
                    camera,
                    size,
                    &marks(Vec::new()),
                    true,
                    at,
                )
                .expect("prepared");
            viewport.draw(&gpu, &mut renderer).expect("drawn");
            (due, viewport.report().draws)
        };
        // Its first image, drawn at once.
        assert_eq!(frame(&mut viewport, 0.0, start), (None, 1));
        // What it shows changes at every frame, a hundredth of a second apart: it draws once its
        // rest has passed, and asks for a frame then.
        let at = |milliseconds| start + std::time::Duration::from_millis(milliseconds);
        assert_eq!(frame(&mut viewport, 1.0, at(10)), (Some(at(100)), 1));
        assert_eq!(frame(&mut viewport, 2.0, at(50)), (Some(at(100)), 1));
        assert_eq!(frame(&mut viewport, 3.0, at(100)), (None, 2));
        // A picking does not wait for it.
        let area = slime_render::Viewport {
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        };
        let _pick = viewport.pick(area);
        assert_eq!(frame(&mut viewport, 3.0, at(110)), (None, 3));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_selected_entity_that_draws_nothing_shows_as_the_box_that_its_transform_places() {
        let (_gpu, _renderer, mut level, _viewport, directory) = viewport("box");
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(
            entity,
            Transform {
                translation: DVec3::new(1.0, 2.0, 3.0),
                scale: 2.0,
                ..Transform::IDENTITY
            },
        );
        let mut drawings = Drawings::default();
        level.mark(
            &world,
            &Scene::new(Vec::new()),
            &mut drawings,
            &marks(vec![entity]),
        );
        assert_eq!(drawings.over_image.lines.len(), 12, "a box's edges");
        assert!(
            drawings.outline.picks.is_empty(),
            "nothing drawn to outline"
        );
        // Two metres a side, around where it stands.
        for line in &drawings.over_image.lines {
            for end in [line.from, line.to] {
                let offset = (end - DVec3::new(1.0, 2.0, 3.0)).abs();
                assert!(offset.abs_diff_eq(DVec3::ONE, 1e-9), "{end}");
            }
            assert_eq!((line.to - line.from).length(), 2.0);
        }
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_wireframe_draws_each_edge_of_a_mesh_once() {
        let positions = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        // Two triangles of a square, which share their diagonal, run each way round.
        let edges = edges_of(&positions, &[0, 1, 2, 0, 2, 3]);
        assert_eq!(edges.len(), 5);
        // An index beyond the positions draws nothing.
        assert!(edges_of(&positions, &[0, 1, 9]).len() == 1);
    }
}
