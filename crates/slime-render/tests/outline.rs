//! The outline that a view draws around instances over its image
//! (`docs/adr/0159-picking-and-the-selection.md`), read back from an output of floats: around what
//! it outlines, in its color where the scene shows it, in its hidden part's where a surface in
//! front hides it, and nothing inside; the same without anti-aliasing and at another render scale;
//! around an instance that a client draws in the scene too, and one of the physically based kind,
//! but not around its back, which the scene culls; and no pass at all when nothing that it names
//! is drawn. Runs on whatever adapter the machine offers.

#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DQuat, DVec3};
use slime_render::{
    AlphaMode, AntiAliasing, Camera, DrawList, Drawings, Instance, MaterialData, Mesh, MeshData,
    Outline, Renderer, Scene, UnlitData, View, ViewDraw, ViewSettings,
};

use tone_curve::tone_map;

const SIZE: u32 = 64;
const FORMAT: TextureFormat = TextureFormat::Rgba16Float;

/// The picking number of the square outlined.
const OUTLINED: u64 = 7;

const GREEN: LinearColor = LinearColor::rgb(0.0, 0.6, 0.0);
const BLUE: LinearColor = LinearColor::rgb(0.0, 0.0, 0.8);
const SEEN: LinearColor = LinearColor::rgba(1.0, 0.5, 0.0, 1.0);
const HIDDEN: LinearColor = LinearColor::rgba(1.0, 0.5, 0.0, 0.5);

struct Drawing {
    gpu: Gpu,
    renderer: Renderer,
    square: Mesh,
}

impl Drawing {
    fn new() -> Self {
        let gpu =
            Gpu::new().expect("a GPU adapter must be available; CONTRIBUTING.md lists the drivers");
        let renderer = Renderer::new(&gpu, FORMAT).expect("the renderer is created");
        let square = renderer
            .create_mesh(&gpu, "square", &MeshData::square())
            .expect("the square is valid");
        Self {
            gpu,
            renderer,
            square,
        }
    }

    /// An unlit square of `color`, a unit wide, centred at `(x, 0, z)`, picked as `pick`.
    fn square(&self, color: LinearColor, x: f64, z: f64, pick: u64) -> Instance {
        let material = self
            .renderer
            .create_unlit_material(
                &self.gpu,
                "square",
                &UnlitData {
                    color,
                    alpha_mode: AlphaMode::Opaque,
                    ..UnlitData::default()
                },
            )
            .expect("the material is valid");
        let mut square = Instance::new(
            &self.square,
            &material,
            DAffine3::from_translation(DVec3::new(x, 0.0, z)),
        );
        square.pick = pick;
        square
    }

    fn view(&self, settings: ViewSettings) -> View {
        let mut view = self
            .renderer
            .create_view(&self.gpu, "view")
            .expect("the view is created");
        view.set_settings(settings);
        view
    }

    /// `scene` and `drawings` seen through `view` from 3 units along +Z, the camera's axis through
    /// the image's middle.
    fn draw(&mut self, scene: &Scene, view: &mut View, drawings: &Drawings) -> FloatImage {
        let target = self
            .gpu
            .create_render_target("outline", SIZE, SIZE, FORMAT)
            .expect("the GPU has memory for it");
        let camera = Camera {
            eye: DVec3::new(0.0, 0.0, 3.0),
            ..Camera::default()
        };
        let mut draw = ViewDraw::new(view, camera, &target);
        draw.drawings = Some(drawings);
        self.renderer
            .render(&self.gpu, scene, &mut [draw])
            .expect("the frame is drawn");
        self.gpu
            .read_float_render_target(&target)
            .expect("the image is read back")
    }

    /// The green square outlined, at the origin, and the blue one in front of its right half, a
    /// unit nearer the camera, not outlined: across the middle row, the green square from x = 22.8
    /// to 41.2 pixels, the blue one from 32 to 59.7.
    fn scene(&self) -> Scene {
        Scene::new([
            self.square(GREEN, 0.0, 0.0, OUTLINED),
            self.square(BLUE, 0.5, 1.0, 1),
        ])
    }
}

/// What outlines the square of [`OUTLINED`], two pixels wide.
fn outlined() -> Drawings {
    Drawings {
        outline: Outline {
            picks: [OUTLINED].into(),
            seen: SEEN,
            hidden: HIDDEN,
            width: 2.0,
        },
        ..Drawings::default()
    }
}

/// The red, green and blue of the pixel at `(x, y)`, `y` counted from the top.
fn pixel(image: &FloatImage, (x, y): (u32, u32)) -> [f32; 3] {
    let start = ((y * image.width + x) * 4) as usize;
    [
        image.pixels[start],
        image.pixels[start + 1],
        image.pixels[start + 2],
    ]
}

/// Whether `shown` is `expected` within half precision's rounding.
fn near(shown: [f32; 3], expected: [f32; 3]) -> bool {
    shown
        .iter()
        .zip(expected)
        .all(|(&shown, expected)| (shown - expected).abs() <= 2e-3)
}

/// `over` laid over `under` by its alpha.
fn laid_over(over: LinearColor, under: [f32; 3]) -> [f32; 3] {
    let [r, g, b] = under;
    [
        over.r * over.a + r * (1.0 - over.a),
        over.g * over.a + g * (1.0 - over.a),
        over.b * over.a + b * (1.0 - over.a),
    ]
}

/// Checks the middle row of `image`: the outline on the left, where the scene shows the square;
/// on the right, over the blue square that hides it; the square's own color inside; and nothing
/// beyond the outline's width.
fn check(image: &FloatImage, case: &str) {
    let row = SIZE / 2;
    let (green, blue) = (tone_map([0.0, 0.6, 0.0]), tone_map([0.0, 0.0, 0.8]));
    let seen = [SEEN.r, SEEN.g, SEEN.b];
    let cases = [
        (20, [0.0; 3], "beyond the outline, on the left"),
        (
            21,
            seen,
            "the outline, two pixels wide, where the square shows",
        ),
        (22, seen, "the outline, beside the square, where it shows"),
        (28, green, "inside the square"),
        (
            41,
            laid_over(HIDDEN, blue),
            "the outline, where the blue square hides it",
        ),
        (
            42,
            laid_over(HIDDEN, blue),
            "the outline, two pixels wide, where the blue square hides it",
        ),
        (43, blue, "beyond the outline, on the right"),
    ];
    for (x, expected, what) in cases {
        let shown = pixel(image, (x, row));
        assert!(
            near(shown, expected),
            "{case}, {what}: {shown:?} where {expected:?}"
        );
    }
}

#[test]
fn the_outline_shows_what_the_scene_hides_in_its_hidden_part_s_color() {
    let mut drawing = Drawing::new();
    let scene = drawing.scene();
    let mut view = drawing.view(ViewSettings::default());
    let image = drawing.draw(&scene, &mut view, &outlined());
    check(&image, "four samples");
    // Its passes timed, once their times have come back.
    drawing.draw(&scene, &mut view, &outlined());
    if drawing.gpu.can_time() {
        assert!(
            view.stats().outline_gpu_time.is_some(),
            "{:?}",
            view.stats()
        );
    }
    let mut settings = ViewSettings::default();
    settings.anti_aliasing = AntiAliasing::Off;
    let mut view = drawing.view(settings);
    let image = drawing.draw(&scene, &mut view, &outlined());
    check(&image, "one sample");
}

#[test]
fn the_outline_keeps_its_width_in_the_output_s_pixels_at_another_render_scale() {
    let mut drawing = Drawing::new();
    let scene = drawing.scene();
    let mut settings = ViewSettings::default();
    settings.render_scale = 2.0;
    let mut view = drawing.view(settings);
    let image = drawing.draw(&scene, &mut view, &outlined());
    let row = SIZE / 2;
    // The two pixels beside the square, as at a render scale of 1, not one of them.
    let seen = [SEEN.r, SEEN.g, SEEN.b];
    assert!(near(pixel(&image, (22, row)), seen));
    assert!(near(pixel(&image, (21, row)), seen));
    assert!(near(pixel(&image, (20, row)), [0.0; 3]));
}

#[test]
fn an_instance_drawn_in_the_scene_by_a_client_is_outlined_as_the_scene_s_are() {
    let mut drawing = Drawing::new();
    let scene = Scene::new([drawing.square(BLUE, 0.5, 1.0, 1)]);
    let drawings = Drawings {
        in_scene: DrawList {
            instances: vec![drawing.square(GREEN, 0.0, 0.0, OUTLINED)],
            ..DrawList::default()
        },
        ..outlined()
    };
    let mut view = drawing.view(ViewSettings::default());
    let image = drawing.draw(&scene, &mut view, &drawings);
    check(&image, "drawn by a client");
}

#[test]
fn a_view_outlines_nothing_when_nothing_that_it_names_is_drawn() {
    let mut drawing = Drawing::new();
    let scene = drawing.scene();
    let mut view = drawing.view(ViewSettings::default());
    let plain = drawing.draw(&scene, &mut view, &Drawings::default());
    let mut nothing = outlined();
    nothing.outline.picks = [OUTLINED + 1, 0].into();
    let image = drawing.draw(&scene, &mut view, &nothing);
    assert_eq!(image.pixels, plain.pixels, "the same image");
    // Drawn again, the times of the frames before have come back: none of the outline's passes.
    drawing.draw(&scene, &mut view, &nothing);
    assert_eq!(view.stats().outline_gpu_time, None, "no outline's pass");
}

#[test]
fn a_lit_surface_is_outlined_and_its_back_that_the_scene_culls_is_not() {
    let mut drawing = Drawing::new();
    let material = drawing
        .renderer
        .create_material(&drawing.gpu, "lit", &MaterialData::default())
        .expect("the material is valid");
    let square = |turned: bool| {
        let mut square = Instance::new(
            &drawing.square,
            &material,
            DAffine3::from_quat(if turned {
                DQuat::from_rotation_y(std::f64::consts::PI)
            } else {
                DQuat::IDENTITY
            }),
        );
        square.pick = OUTLINED;
        Scene::new([square])
    };
    let (facing, turned) = (square(false), square(true));
    let mut view = drawing.view(ViewSettings::default());
    let row = SIZE / 2;
    let image = drawing.draw(&facing, &mut view, &outlined());
    assert!(near(pixel(&image, (22, row)), [SEEN.r, SEEN.g, SEEN.b]));
    // Turned away, single-sided: the scene draws nothing of it, nor does the outline.
    let plain = drawing.draw(&turned, &mut view, &Drawings::default());
    let image = drawing.draw(&turned, &mut view, &outlined());
    assert_eq!(image.pixels, plain.pixels);
}
