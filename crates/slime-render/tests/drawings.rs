//! What a client draws into a view besides the scene (`docs/adr/0129-drawing-for-the-editor.md`),
//! read back from an output of floats: a line over the image is its width in pixels wide, in its
//! color as given, over everything; a line in the scene is hidden by what is in front of it, and
//! shown through the tone curve; its width stays in the output's pixels at any render scale; the
//! instances drawn in the scene are hidden by what is in front of them, and a blended one is laid
//! over what is behind it; a line on the ground that runs away from the camera keeps its width all
//! along; a view drawn into a rectangle of its output draws its lines over the image there; a line that runs behind the camera draws its part in front; the
//! drawings' passes run only in the views that have drawings. Runs on whatever adapter the machine
//! offers.

#[path = "common/tone_curve.rs"]
#[expect(
    dead_code,
    reason = "this file reads float outputs: it has no use for the sRGB encoding"
)]
mod tone_curve;

use slime_color::LinearColor;
use slime_gfx::{FloatImage, Gpu, TextureFormat};
use slime_render::glam::{DAffine3, DVec3};
use slime_render::{
    AlphaMode, Camera, DrawList, Drawings, Instance, Line, Mesh, MeshData, Renderer, Scene,
    UnlitData, View, ViewDraw, ViewSettings, Viewport,
};

use tone_curve::tone_map;

const SIZE: u32 = 64;
const FORMAT: TextureFormat = TextureFormat::Rgba16Float;

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

    /// An unlit square of `color`, `side` units wide, at `z` along the camera's axis.
    fn square(&self, color: LinearColor, alpha_mode: AlphaMode, side: f64, z: f64) -> Instance {
        let material = self
            .renderer
            .create_unlit_material(
                &self.gpu,
                "square",
                &UnlitData {
                    color,
                    alpha_mode,
                    ..UnlitData::default()
                },
            )
            .expect("the material is valid");
        Instance::new(
            &self.square,
            &material,
            DAffine3::from_translation(DVec3::new(0.0, 0.0, z))
                * DAffine3::from_scale(DVec3::splat(side)),
        )
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
            .create_render_target("drawings", SIZE, SIZE, FORMAT)
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

/// A line across the whole image, through its middle, `width` pixels wide, at `z`.
fn across(color: LinearColor, width: f32, z: f64) -> Line {
    Line {
        from: DVec3::new(-20.0, 0.0, z),
        to: DVec3::new(20.0, 0.0, z),
        color,
        width,
    }
}

/// How many of column `x`'s pixels have red above one half.
fn reddened(image: &FloatImage, x: u32) -> usize {
    (0..image.height)
        .filter(|&y| pixel(image, (x, y))[0] > 0.5)
        .count()
}

const RED: LinearColor = LinearColor::rgb(1.0, 0.0, 0.0);
const GREEN: LinearColor = LinearColor::rgb(0.0, 0.6, 0.0);

#[test]
fn a_line_over_the_image_is_its_width_in_pixels_wide_in_its_color_over_everything() {
    let mut drawing = Drawing::new();
    let scene = Scene::new([drawing.square(GREEN, AlphaMode::Opaque, 2.0, 0.0)]);
    let mut view = drawing.view(ViewSettings::default());
    let drawings = Drawings {
        over_image: vec![across(RED, 4.0, -1.0)],
        ..Drawings::default()
    };
    let image = drawing.draw(&scene, &mut view, &drawings);
    // Through the square and beside it: the image's middle lies between two rows, and the line
    // covers the two above and the two below whole.
    for x in [5, SIZE / 2] {
        assert_eq!(reddened(&image, x), 4, "column {x}");
        for y in SIZE / 2 - 2..SIZE / 2 + 2 {
            assert!(near(pixel(&image, (x, y)), [1.0, 0.0, 0.0]), "{x}, {y}");
        }
    }
}

#[test]
fn a_line_in_the_scene_is_hidden_by_what_is_in_front_of_it_and_shown_through_the_tone_curve() {
    let mut drawing = Drawing::new();
    let scene = Scene::new([drawing.square(GREEN, AlphaMode::Opaque, 2.0, 0.0)]);
    let mut view = drawing.view(ViewSettings::default());
    let color = LinearColor::rgb(0.9, 0.2, 0.1);
    let drawings = Drawings {
        in_scene: DrawList {
            lines: vec![across(color, 4.0, -1.0)],
            ..DrawList::default()
        },
        ..Drawings::default()
    };
    let image = drawing.draw(&scene, &mut view, &drawings);
    let row = SIZE / 2;
    let line = tone_map([0.9, 0.2, 0.1]);
    let square = tone_map([0.0, 0.6, 0.0]);
    let (beside, behind) = (pixel(&image, (5, row)), pixel(&image, (SIZE / 2, row)));
    assert!(near(beside, line), "{beside:?} where {line:?}");
    assert!(near(behind, square), "{behind:?} where {square:?}");
    // In front of the square, it shows over it.
    let drawings = Drawings {
        in_scene: DrawList {
            lines: vec![across(color, 4.0, 1.0)],
            ..DrawList::default()
        },
        ..Drawings::default()
    };
    let image = drawing.draw(&scene, &mut view, &drawings);
    let in_front = pixel(&image, (SIZE / 2, row));
    assert!(near(in_front, line), "{in_front:?} where {line:?}");
}

#[test]
fn a_line_keeps_its_width_in_the_output_s_pixels_at_any_render_scale() {
    let mut drawing = Drawing::new();
    let scene = Scene::new(Vec::new());
    for render_scale in [0.5, 1.0, 2.0] {
        let mut settings = ViewSettings::default();
        settings.render_scale = render_scale;
        let mut view = drawing.view(settings);
        let drawings = Drawings {
            in_scene: DrawList {
                lines: vec![across(RED, 6.0, 0.0)],
                ..DrawList::default()
            },
            ..Drawings::default()
        };
        let image = drawing.draw(&scene, &mut view, &drawings);
        let rows = reddened(&image, SIZE / 2);
        assert!((5..=7).contains(&rows), "{rows} rows at {render_scale}");
    }
}

#[test]
fn instances_drawn_in_the_scene_are_hidden_by_what_is_in_front_and_blend_over_what_is_behind() {
    let mut drawing = Drawing::new();
    let scene = Scene::new([drawing.square(GREEN, AlphaMode::Opaque, 2.0, 0.0)]);
    let mut view = drawing.view(ViewSettings::default());
    // A half-transparent square, wider, behind the green one.
    let icon = drawing.square(
        LinearColor::rgba(0.0, 0.0, 0.8, 0.5),
        AlphaMode::Blend,
        4.0,
        -1.0,
    );
    let drawings = Drawings {
        in_scene: DrawList {
            instances: vec![icon],
            ..DrawList::default()
        },
        ..Drawings::default()
    };
    let image = drawing.draw(&scene, &mut view, &drawings);
    let row = SIZE / 2;
    let (beside, behind) = (pixel(&image, (8, row)), pixel(&image, (SIZE / 2, row)));
    let blue = tone_map([0.0, 0.0, 0.4]);
    let green = tone_map([0.0, 0.6, 0.0]);
    assert!(near(beside, blue), "{beside:?} where {blue:?}");
    assert!(near(behind, green), "{behind:?} where {green:?}");
}

#[test]
fn a_line_on_the_ground_that_runs_away_from_the_camera_keeps_its_width_all_along() {
    let mut drawing = Drawing::new();
    let scene = Scene::new(Vec::new());
    let mut view = drawing.view(ViewSettings::default());
    // A unit below the camera, from beneath it to far ahead: from the image's bottom edge up to near
    // its middle, where the horizon lies, down the middle column.
    let drawings = Drawings {
        in_scene: DrawList {
            lines: vec![Line {
                from: DVec3::new(0.0, -1.0, 2.0),
                to: DVec3::new(0.0, -1.0, -40.0),
                color: RED,
                width: 2.0,
            }],
            ..DrawList::default()
        },
        ..Drawings::default()
    };
    let image = drawing.draw(&scene, &mut view, &drawings);
    for y in SIZE / 2 + 3..SIZE {
        let row: Vec<f32> = (SIZE / 2 - 2..SIZE / 2 + 2)
            .map(|x| pixel(&image, (x, y))[0])
            .collect();
        // The two columns either side of the middle, whole: red, through the tone curve.
        let whole = tone_map([1.0, 0.0, 0.0])[0] - 0.01;
        assert!(row[1] > whole && row[2] > whole, "row {y}: {row:?}");
    }
}

#[test]
fn a_view_drawn_into_a_rectangle_draws_its_lines_over_the_image_there() {
    let mut drawing = Drawing::new();
    let scene = Scene::new(Vec::new());
    let mut view = drawing.view(ViewSettings::default());
    let target = drawing
        .gpu
        .create_render_target("halves", SIZE, SIZE, FORMAT)
        .expect("the GPU has memory for it");
    let drawings = Drawings {
        over_image: vec![across(RED, 4.0, 0.0)],
        ..Drawings::default()
    };
    // The right half, half as wide as it is high: the line across it, through its middle.
    let mut draw = ViewDraw::new(
        &mut view,
        Camera {
            eye: DVec3::new(0.0, 0.0, 3.0),
            ..Camera::default()
        },
        &target,
    );
    draw.viewport = Some(Viewport {
        x: SIZE / 2,
        y: 0,
        width: SIZE / 2,
        height: SIZE,
    });
    draw.drawings = Some(&drawings);
    drawing
        .renderer
        .render(&drawing.gpu, &scene, &mut [draw])
        .expect("the frame is drawn");
    let image = drawing
        .gpu
        .read_float_render_target(&target)
        .expect("the image is read back");
    assert_eq!(reddened(&image, SIZE / 4), 0, "the left half untouched");
    assert_eq!(reddened(&image, SIZE * 3 / 4), 4);
}

#[test]
fn a_line_that_runs_behind_the_camera_draws_its_part_in_front_of_it() {
    let mut drawing = Drawing::new();
    let scene = Scene::new(Vec::new());
    let mut view = drawing.view(ViewSettings::default());
    // From ahead of the camera, on the left, to far behind it.
    let line = Line {
        from: DVec3::new(-1.0, 0.0, 0.0),
        to: DVec3::new(-1.0, 0.0, 100.0),
        color: RED,
        width: 2.0,
    };
    for drawings in [
        Drawings {
            over_image: vec![line],
            ..Drawings::default()
        },
        Drawings {
            in_scene: DrawList {
                lines: vec![line],
                ..DrawList::default()
            },
            ..Drawings::default()
        },
    ] {
        let image = drawing.draw(&scene, &mut view, &drawings);
        // Its end in front, 3 units away, a unit to the left: about 18 pixels left of the middle.
        let shown: Vec<u32> = (0..SIZE)
            .filter(|&x| pixel(&image, (x, SIZE / 2))[0] > 0.1)
            .collect();
        assert!(
            shown.first() == Some(&0) && shown.contains(&13),
            "{shown:?}"
        );
        assert!(
            image.pixels.iter().all(|value| value.is_finite()),
            "nothing but numbers"
        );
    }
}

#[test]
fn the_drawings_passes_run_only_in_the_views_that_have_drawings() {
    let mut drawing = Drawing::new();
    let scene = Scene::new([drawing.square(GREEN, AlphaMode::Opaque, 2.0, 0.0)]);
    let mut view = drawing.view(ViewSettings::default());
    let without = drawing.draw(&scene, &mut view, &Drawings::default());
    let drawings = Drawings {
        over_image: vec![across(RED, 2.0, 0.0)],
        ..Drawings::default()
    };
    let with = drawing.draw(&scene, &mut view, &drawings);
    let empty = drawing.draw(&scene, &mut view, &Drawings::default());
    assert_ne!(with.pixels, without.pixels);
    assert_eq!(empty.pixels, without.pixels);
}
