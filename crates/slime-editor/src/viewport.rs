//! The viewport: the level drawn by the editor's own asset cache and frame builder into an image
//! that the window's surface shows, drawn again only when what it shows changed
//! (`docs/adr/0158-the-viewport.md`). Its camera moves as [`Navigation`](crate::navigation)
//! says; the grid on the ground and the icons of the lights are drawn into it.

use std::time::Instant;

use slime_assets::CookedAssets;
use slime_color::LinearColor;
use slime_gfx::{AddressMode, Filter, Gpu, RenderTarget, SamplerDescriptor, TextureFormat};
use slime_reflect::Reflect;
use slime_render::glam::DVec3;
use slime_render::{
    AlphaMode, AssetCache, Camera, DrawList, Drawings, Instance, Light, Line, Material, Mesh,
    MeshData, RenderError, RenderStats, Renderer, SampledTexture, Scene, UnlitData, View, ViewDraw,
    ViewSettings, facing,
};
use slime_ui::display::ImageKey;
use slime_ui::look;
use slime_ui::text::{Fonts, GlyphPixels, GlyphRasterizer, TextBounds, TextStyle};
use slime_world::{Entity, Tick, World};
use slime_world_render::FrameBuilder;

use crate::program::Lighting;

/// The key of the viewport's image in the window's display list, which the window's surface shows.
pub(crate) const IMAGE: ImageKey = ImageKey(1);

/// The name by which tools capture the viewport and change its settings.
pub(crate) const NAME: &str = "viewport";

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

/// What the image shows: when it differs, the image is drawn again.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Shown {
    tick: Tick,
    camera: Camera,
    size: (u32, u32),
    settings: ViewSettings,
}

/// The viewport's view, its image, and what draws the level into it, on one GPU device.
pub(crate) struct Viewport {
    view: View,
    cache: AssetCache,
    builder: FrameBuilder,
    icons: Icons,
    lighting: Lighting,
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
}

impl Viewport {
    /// A viewport that `renderer` draws, in its format, the level's meshes loaded from `assets`, lit
    /// as `lighting` says.
    ///
    /// # Errors
    ///
    /// When the GPU cannot make the view or the icons.
    pub(crate) fn new(
        gpu: &Gpu,
        renderer: &Renderer,
        assets: CookedAssets,
        lighting: Lighting,
        format: TextureFormat,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            view: renderer.create_view(gpu, NAME)?,
            cache: AssetCache::new(renderer, assets),
            builder: FrameBuilder::new(lighting.sky),
            icons: Icons::new(gpu, renderer)?,
            lighting,
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

    /// Loads from `assets` from now on, the cooked data of a level opened since: what did not
    /// change is kept ([`AssetCache::reload`]), and the image is drawn again.
    pub(crate) fn reload(&mut self, gpu: &Gpu, assets: CookedAssets) {
        self.cache.reload(gpu, assets);
        self.shown = None;
    }

    /// What it reports to tools: how many times it drew its image, and what the last took.
    pub(crate) fn report(&self) -> Report {
        Report {
            draws: self.draws,
            stats: self.view.stats(),
        }
    }

    /// Makes ready what the next [`Viewport::draw`] draws of `world` from `camera`, at `size` in
    /// pixels, at `now`, if what the image shows changed: the world, the camera, the size or the
    /// settings, a mesh that came since the last frame, or a new size that has held still
    /// ([`View::resize_due`]). Gives whether it waits for meshes still loading, for which the
    /// window asks for the next frame.
    ///
    /// # Errors
    ///
    /// When the GPU cannot make the image, or the frame cannot be built
    /// ([`FrameBuilder::build`]).
    pub(crate) fn prepare(
        &mut self,
        gpu: &Gpu,
        world: &World,
        camera: Camera,
        size: (u32, u32),
        now: Instant,
    ) -> Result<bool, RenderError> {
        if self
            .image
            .as_ref()
            .is_none_or(|image| (image.width(), image.height()) != size)
        {
            self.image = Some(gpu.create_render_target("viewport", size.0, size.1, self.format)?);
            self.shown = None;
        }
        let shown = Shown {
            tick: world.tick(),
            camera,
            size,
            settings: self.settings,
        };
        let settled = self.view.resize_due().is_some_and(|due| due <= now);
        if self.shown == Some(shown) && !self.loading && !settled {
            return Ok(false);
        }
        let frame = self.builder.build(world, &mut self.cache, gpu, 1.0)?;
        self.loading = frame.loading > 0;
        let icons = frame
            .lights
            .iter()
            .filter_map(|&(entity, light)| {
                let (position, material) = match light {
                    Light::Point(light) => (light.position, &self.icons.point),
                    Light::Spot(light) => (light.position, &self.icons.spot),
                    // A directional light shows where its entity stands, the sky's sun nowhere.
                    Light::Directional(_) => (
                        world.interpolated_transform(entity, 1.0)?.translation,
                        &self.icons.directional,
                    ),
                };
                // Above the light, out of the lamp that holds it.
                let above = position + DVec3::Y * ICON_SIDE;
                let mut icon = Instance::new(
                    &self.icons.square,
                    material,
                    facing(above, camera.eye, ICON_SIDE),
                );
                icon.casts_shadows = false;
                Some(icon)
            })
            .collect();
        let drawings = Drawings {
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
        scene.exposure = self.lighting.exposure;
        scene.bloom = self.lighting.bloom;
        self.pending = Some((scene, camera, drawings));
        self.shown = Some(shown);
        Ok(self.loading)
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
