//! Picking by the GPU (`docs/adr/0130-picking.md`): a picking names, at each pixel of the area
//! asked for, the instance drawn nearest there, by the number that its client gave it, 0 where none
//! is, in an area off the view's middle too, the view drawn into a part of its output at another
//! render scale; it passes through what a mask cuts out, unlit or physically based, and picks a
//! blended surface where its alpha reaches one half; its answer comes back a frame or more later,
//! never at once, so that no frame waits for it; an area outside the view gives an empty answer; on
//! a lost device, it gives the loss, and no panic. Runs on whatever adapter the machine offers.

use std::time::Duration;

use slime_color::LinearColor;
use slime_gfx::{
    AddressMode, Filter, GfxError, Gpu, Sampler, SamplerDescriptor, Texture, TextureFormat,
};
use slime_render::glam::{DAffine3, DVec3};
use slime_render::{
    AlphaMode, Camera, Instance, MaterialData, Mesh, MeshData, Pick, Picked, RenderError, Renderer,
    SampledTexture, Scene, UnlitData, View, ViewDraw, ViewSettings, Viewport,
};

const SIZE: u32 = 32;

struct Picking {
    gpu: Gpu,
    renderer: Renderer,
    view: View,
    square: Mesh,
    /// The size of the output that the view draws into, and its rectangle there, the whole output
    /// for `None`.
    output: ((u32, u32), Option<Viewport>),
}

impl Picking {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer =
            Renderer::new(&gpu, TextureFormat::Rgba8UnormSrgb).expect("the renderer is created");
        let view = renderer
            .create_view(&gpu, "view")
            .expect("the view is created");
        let square = renderer
            .create_mesh(&gpu, "square", &MeshData::square())
            .expect("the square is valid");
        Self {
            gpu,
            renderer,
            view,
            square,
            output: ((SIZE, SIZE), None),
        }
    }

    /// A square of `data`, `side` units wide, at `z` along the camera's axis, picked as `number`.
    fn square(&self, data: &UnlitData<'_>, side: f64, z: f64, number: u64) -> Instance {
        let material = self
            .renderer
            .create_unlit_material(&self.gpu, "square", data)
            .expect("the material is valid");
        let mut instance = Instance::new(
            &self.square,
            &material,
            DAffine3::from_translation(DVec3::new(0.0, 0.0, z))
                * DAffine3::from_scale(DVec3::splat(side)),
        );
        instance.pick = number;
        instance
    }

    /// Draws `scene` from 3 units along +Z once, the camera's axis through the image's middle.
    fn draw(&mut self, scene: &Scene) {
        let ((width, height), viewport) = self.output;
        let target = self
            .gpu
            .create_render_target("picking", width, height, TextureFormat::Rgba8UnormSrgb)
            .expect("the GPU has memory for it");
        let camera = Camera {
            eye: DVec3::new(0.0, 0.0, 3.0),
            ..Camera::default()
        };
        let mut draw = ViewDraw::new(&mut self.view, camera, &target);
        draw.viewport = viewport;
        self.renderer
            .render(&self.gpu, scene, &mut [draw])
            .expect("the frame is drawn");
    }

    /// A texture opaque on its left half and cut out on its right, read texel by texel.
    fn half_cut(&self) -> (Texture, Sampler) {
        let texture = self
            .gpu
            .create_texture(
                "half",
                2,
                1,
                TextureFormat::Rgba8UnormSrgb,
                &[255, 255, 255, 255, 255, 255, 255, 0],
            )
            .expect("the GPU has memory for it");
        let sampler = self
            .gpu
            .create_sampler(
                "nearest",
                &SamplerDescriptor {
                    mag_filter: Filter::Nearest,
                    min_filter: Filter::Nearest,
                    address_u: AddressMode::ClampToEdge,
                    address_v: AddressMode::ClampToEdge,
                },
            )
            .expect("the GPU has memory for it");
        (texture, sampler)
    }

    /// Draws `scene` frame after frame until `pick` answers, for up to ten seconds.
    fn answer(&mut self, scene: &Scene, pick: &Pick) -> Result<Picked, RenderError> {
        for _ in 0..10_000 {
            if let Some(answer) = pick.take(&self.gpu) {
                return answer;
            }
            self.draw(scene);
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("the picking did not come back in ten seconds of frames");
    }
}

/// The whole view.
const WHOLE: Viewport = Viewport {
    x: 0,
    y: 0,
    width: SIZE,
    height: SIZE,
};

/// A color, opaque.
fn opaque(r: f32, g: f32, b: f32) -> UnlitData<'static> {
    UnlitData {
        color: LinearColor::rgb(r, g, b),
        ..UnlitData::default()
    }
}

#[test]
fn a_picking_names_the_instance_drawn_nearest_at_each_pixel() {
    let mut picking = Picking::new();
    // A large square behind a small one, the small one listed last.
    let scene = Scene::new([
        picking.square(&opaque(0.0, 0.0, 1.0), 2.0, -1.0, 2),
        picking.square(&opaque(1.0, 0.0, 0.0), 0.5, 0.0, 1 << 40 | 7),
    ]);
    let pick = picking.view.pick(WHOLE);
    let picked = picking.answer(&scene, &pick).expect("the picking answers");
    assert_eq!((picked.width, picked.height), (SIZE, SIZE));
    assert_eq!(picked.numbers.len(), (SIZE * SIZE) as usize);
    // The small square in the middle, all 64 bits of its number; the large one around it; nothing
    // in the corners.
    assert_eq!(picked.at(SIZE / 2, SIZE / 2), 1 << 40 | 7);
    assert_eq!(picked.at(SIZE / 2 - 5, SIZE / 2), 2);
    assert_eq!(picked.at(0, 0), 0);
    assert!(pick.take(&picking.gpu).is_none(), "an answer is given once");
    // An area of the view: its pixels from its top left corner.
    let pick = picking.view.pick(Viewport {
        x: SIZE / 2 - 1,
        y: SIZE / 2 - 1,
        width: 2,
        height: 2,
    });
    let picked = picking.answer(&scene, &pick).expect("the picking answers");
    assert_eq!(picked.numbers, [1 << 40 | 7; 4]);
}

#[test]
fn a_picking_passes_through_what_a_mask_cuts_out_and_takes_a_blend_from_one_half() {
    let mut picking = Picking::new();
    // Kept on the left, cut out on the right.
    let (texture, sampler) = picking.half_cut();
    let sampled = SampledTexture {
        texture: &texture,
        sampler: &sampler,
    };
    let behind = picking.square(&opaque(0.0, 0.0, 1.0), 2.0, -1.0, 1);
    let left = (SIZE / 2 - 4, SIZE / 2);
    let right = (SIZE / 2 + 4, SIZE / 2);
    for (alpha_mode, color) in [
        (AlphaMode::Mask { cutoff: 0.5 }, LinearColor::WHITE),
        (AlphaMode::Blend, LinearColor::WHITE),
        // Too faint to be picked anywhere.
        (AlphaMode::Blend, LinearColor::rgba(1.0, 1.0, 1.0, 0.4)),
    ] {
        let front = picking.square(
            &UnlitData {
                color,
                texture: Some(sampled),
                alpha_mode,
                ..UnlitData::default()
            },
            1.5,
            0.0,
            2,
        );
        let scene = Scene::new([behind.clone(), front]);
        let pick = picking.view.pick(WHOLE);
        let picked = picking.answer(&scene, &pick).expect("the picking answers");
        let kept = if color.a >= 0.5 { 2 } else { 1 };
        assert_eq!(picked.at(left.0, left.1), kept, "{alpha_mode:?}, {color:?}");
        assert_eq!(picked.at(right.0, right.1), 1, "{alpha_mode:?}, {color:?}");
    }
    // A physically based mask, its cut through its own shader.
    let material = picking
        .renderer
        .create_material(
            &picking.gpu,
            "masked",
            &MaterialData {
                base_color_texture: Some(sampled),
                alpha_mode: AlphaMode::Mask { cutoff: 0.5 },
                ..MaterialData::default()
            },
        )
        .expect("the material is valid");
    let mut front = behind.clone();
    front.material = material;
    front.transform = DAffine3::from_scale(DVec3::splat(1.5));
    front.pick = 3;
    let scene = Scene::new([behind, front]);
    let pick = picking.view.pick(WHOLE);
    let picked = picking.answer(&scene, &pick).expect("the picking answers");
    assert_eq!(picked.at(left.0, left.1), 3);
    assert_eq!(picked.at(right.0, right.1), 1);
}

/// The view drawn into the right half of an output twice as wide, at half the render scale: a
/// picking of one pixel off its middle finds what is drawn there, in the output's pixels.
#[test]
fn a_picking_off_the_middle_of_a_view_drawn_into_a_part_of_its_output_finds_what_is_there() {
    let mut picking = Picking::new();
    let mut settings = ViewSettings::default();
    settings.render_scale = 0.5;
    picking.view.set_settings(settings);
    picking.output = (
        (2 * SIZE, SIZE),
        Some(Viewport {
            x: SIZE,
            y: 0,
            width: SIZE,
            height: SIZE,
        }),
    );
    // Unlike each other along both axes: the left square higher than the right one.
    let at = |x: f64, y: f64, number: u64| {
        let mut square = picking.square(&opaque(1.0, 1.0, 1.0), 1.0, 0.0, number);
        square.transform = DAffine3::from_translation(DVec3::new(x, y, 0.0));
        square
    };
    let scene = Scene::new([at(-0.8, 0.4, 1), at(0.8, 0.0, 2)]);
    let one = |x: u32, y: u32| Viewport {
        x,
        y,
        width: 1,
        height: 1,
    };
    for (at, number) in [((8, 9), 1), ((24, SIZE / 2), 2), ((8, 23), 0)] {
        let pick = picking.view.pick(one(at.0, at.1));
        let picked = picking.answer(&scene, &pick).expect("the picking answers");
        assert_eq!(picked.numbers, [number], "at {at:?}");
    }
}

#[test]
fn a_picking_comes_back_a_frame_or_more_later_and_never_at_once() {
    let mut picking = Picking::new();
    let scene = Scene::new([picking.square(&opaque(1.0, 0.0, 0.0), 2.0, 0.0, 9)]);
    let pick = picking.view.pick(WHOLE);
    assert!(
        pick.take(&picking.gpu).is_none(),
        "answered before any frame"
    );
    picking.draw(&scene);
    // Drawn, its copy submitted: nothing submitted since, so wgpu has not seen it done.
    assert!(
        pick.take(&picking.gpu).is_none(),
        "answered by the frame that drew it"
    );
    let picked = picking.answer(&scene, &pick).expect("the picking answers");
    assert_eq!(picked.at(SIZE / 2, SIZE / 2), 9);
}

#[test]
fn an_area_outside_the_view_gives_an_empty_answer_and_one_across_its_edge_is_cut() {
    let mut picking = Picking::new();
    let scene = Scene::new([picking.square(&opaque(1.0, 0.0, 0.0), 2.0, 0.0, 9)]);
    let outside = picking.view.pick(Viewport {
        x: SIZE,
        y: 0,
        width: 4,
        height: 4,
    });
    assert_eq!(
        picking.answer(&scene, &outside).expect("answered"),
        Picked::default()
    );
    let across = picking.view.pick(Viewport {
        x: SIZE - 2,
        y: SIZE - 2,
        width: 8,
        height: 8,
    });
    let picked = picking.answer(&scene, &across).expect("answered");
    assert_eq!((picked.width, picked.height), (2, 2));
}

#[test]
fn a_device_lost_while_picking_gives_the_loss_and_no_panic() {
    let mut picking = Picking::new();
    let scene = Scene::new([picking.square(&opaque(1.0, 0.0, 0.0), 2.0, 0.0, 9)]);
    // One drawn, its copy on its way back, and one not drawn yet.
    let drawn = picking.view.pick(WHOLE);
    picking.draw(&scene);
    let waiting = picking.view.pick(WHOLE);
    picking.gpu.lose();
    for pick in [&drawn, &waiting] {
        let Some(Err(error)) = pick.take(&picking.gpu) else {
            panic!("a picking of a lost device answered, or said nothing");
        };
        assert!(
            matches!(error, RenderError::Gfx(GfxError::DeviceLost { .. })),
            "{error}"
        );
    }
}
