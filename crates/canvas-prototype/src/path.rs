//! The path's scene: twenty filled shapes of quadratic and cubic Bézier curves, stars, circles,
//! rings, outlines like glyphs and flowers, each flattened on the CPU into at most 64 segments.
//!
//! This counts the data a path takes through each layout, segments kept as variable data and
//! read by the fragment shader; it does not choose how uwui draws vector shapes, which stays open
//! for uwui's first stage (ADR-0242).

use crate::canvas::{Canvas, CanvasBox, LinearColor, PixelRect};

/// The most segments a shape is flattened into.
pub const MOST_SEGMENTS: usize = 64;

type Point = [f32; 2];

enum Curve {
    Line(Point),
    Quadratic(Point, Point),
    Cubic(Point, Point, Point),
}

/// A closed contour: its start, then curves each from the end of the one before.
struct Contour {
    start: Point,
    curves: Vec<Curve>,
}

/// Bézier's handle that draws a quarter of a circle with a cubic.
const ARC: f32 = 0.552_284_8;

/// An ellipse of radii `rx`, `ry` turned by `turn` degrees, clockwise on the screen if `clockwise`.
fn ellipse(rx: f32, ry: f32, turn: f32, clockwise: bool) -> Contour {
    let (sin, cos) = turn.to_radians().sin_cos();
    let at = |x: f32, y: f32| {
        let (x, y) = (x * rx, y * ry);
        [x * cos - y * sin, x * sin + y * cos]
    };
    let sign = if clockwise { 1.0 } else { -1.0 };
    let mut curves = Vec::new();
    for quarter in 0..4 {
        let angle = |q: i32| sign * q as f32 * std::f32::consts::FRAC_PI_2;
        let (s0, c0) = angle(quarter).sin_cos();
        let (s1, c1) = angle(quarter + 1).sin_cos();
        curves.push(Curve::Cubic(
            at(c0 - sign * ARC * s0, s0 + sign * ARC * c0),
            at(c1 + sign * ARC * s1, s1 - sign * ARC * c1),
            at(c1, s1),
        ));
    }
    Contour {
        start: at(1.0, 0.0),
        curves,
    }
}

/// A star of `points` points, its inner points at `inner` of its radius, each edge a quadratic
/// that bows by `bow`.
fn star(points: usize, inner: f32, bow: f32) -> Contour {
    let corner = |index: usize| {
        let angle =
            index as f32 * std::f32::consts::PI / points as f32 - std::f32::consts::FRAC_PI_2;
        let radius = if index % 2 == 0 { 1.0 } else { inner };
        [radius * angle.cos(), radius * angle.sin()]
    };
    let curves = (1..=2 * points)
        .map(|index| {
            let (from, to) = (corner(index - 1), corner(index % (2 * points)));
            let middle = [(from[0] + to[0]) / 2.0, (from[1] + to[1]) / 2.0];
            Curve::Quadratic([middle[0] * bow, middle[1] * bow], to)
        })
        .collect();
    Contour {
        start: corner(0),
        curves,
    }
}

/// A flower of `petals` petals, each a quadratic from the inner circle out to `reach`.
fn flower(petals: usize, reach: f32) -> Contour {
    let at = |angle: f32, radius: f32| [radius * angle.cos(), radius * angle.sin()];
    let step = std::f32::consts::TAU / petals as f32;
    let curves = (1..=petals)
        .map(|index| {
            let angle = index as f32 * step;
            Curve::Quadratic(at(angle - step / 2.0, reach), at(angle, 0.45))
        })
        .collect();
    Contour {
        start: at(0.0, 0.45),
        curves,
    }
}

fn cubic(c1: Point, c2: Point, end: Point) -> Curve {
    Curve::Cubic(c1, c2, end)
}

/// A capital D: its bowl, and its counter turning the other way, a hole.
fn letter_d() -> Vec<Contour> {
    vec![
        Contour {
            start: [-0.6, -0.9],
            curves: vec![
                Curve::Line([0.0, -0.9]),
                cubic([0.6, -0.9], [0.8, -0.4], [0.8, 0.0]),
                cubic([0.8, 0.4], [0.6, 0.9], [0.0, 0.9]),
                Curve::Line([-0.6, 0.9]),
            ],
        },
        Contour {
            start: [-0.3, -0.6],
            curves: vec![
                Curve::Line([-0.3, 0.6]),
                Curve::Line([0.0, 0.6]),
                cubic([0.4, 0.6], [0.5, 0.3], [0.5, 0.0]),
                cubic([0.5, -0.3], [0.4, -0.6], [0.0, -0.6]),
            ],
        },
    ]
}

fn heart() -> Contour {
    Contour {
        start: [0.0, 0.9],
        curves: vec![
            cubic([-0.2, 0.7], [-1.0, 0.3], [-0.9, -0.3]),
            cubic([-0.8, -0.9], [-0.1, -0.9], [0.0, -0.45]),
            cubic([0.1, -0.9], [0.8, -0.9], [0.9, -0.3]),
            cubic([1.0, 0.3], [0.2, 0.7], [0.0, 0.9]),
        ],
    }
}

fn drop() -> Contour {
    Contour {
        start: [0.0, -0.95],
        curves: vec![
            cubic([0.3, -0.5], [0.75, -0.05], [0.75, 0.3]),
            cubic([0.75, 0.75], [0.35, 0.95], [0.0, 0.95]),
            cubic([-0.35, 0.95], [-0.75, 0.75], [-0.75, 0.3]),
            cubic([-0.75, -0.05], [-0.3, -0.5], [0.0, -0.95]),
        ],
    }
}

fn crescent() -> Contour {
    Contour {
        start: [0.0, -1.0],
        curves: vec![
            cubic([-0.55, -1.0], [-1.0, -0.55], [-1.0, 0.0]),
            cubic([-1.0, 0.55], [-0.55, 1.0], [0.0, 1.0]),
            cubic([-0.35, 0.7], [-0.55, 0.4], [-0.55, 0.0]),
            cubic([-0.55, -0.4], [-0.35, -0.7], [0.0, -1.0]),
        ],
    }
}

/// The twenty shapes, each of contours in a square from -1 to 1.
fn shapes() -> Vec<Vec<Contour>> {
    let mut shapes: Vec<Vec<Contour>> = Vec::new();
    for (points, inner, bow) in [
        (5, 0.45, 1.25),
        (6, 0.55, 0.8),
        (7, 0.5, 1.1),
        (8, 0.62, 0.9),
    ] {
        shapes.push(vec![star(points, inner, bow)]);
    }
    for (rx, ry, turn) in [
        (1.0, 1.0, 0.0),
        (1.0, 0.6, 0.0),
        (1.0, 0.55, 30.0),
        (0.5, 1.0, 0.0),
    ] {
        shapes.push(vec![ellipse(rx, ry, turn, true)]);
    }
    for (outer, inner) in [(1.0, 0.6), (1.0, 0.85), (0.9, 0.3), (1.0, 0.45)] {
        shapes.push(vec![
            ellipse(outer, outer, 0.0, true),
            ellipse(inner, inner * 0.8, 20.0, false),
        ]);
    }
    shapes.push(letter_d());
    shapes.push(vec![heart()]);
    shapes.push(vec![drop()]);
    shapes.push(vec![crescent()]);
    for (petals, reach) in [(4, 1.3), (5, 1.25), (6, 1.2), (8, 1.15)] {
        shapes.push(vec![flower(petals, reach)]);
    }
    shapes
}

/// How many segments a curve deserves, from its control polygon: the square root of its length
/// times its turning. An arc of radius r turning by φ, cut into n segments, strays from its chords
/// by r φ² / 8 n²: an even error asks n in proportion to φ √r, which this weight is.
fn weight(start: Point, curve: &Curve) -> f32 {
    let points: &[Point] = match curve {
        Curve::Line(end) => &[*end],
        Curve::Quadratic(control, end) => &[*control, *end],
        Curve::Cubic(c1, c2, end) => &[*c1, *c2, *end],
    };
    let mut from = start;
    let mut length = 0.0;
    let mut turn = 0.0;
    let mut heading: Option<f32> = None;
    for &point in points {
        let (dx, dy) = (point[0] - from[0], point[1] - from[1]);
        length += dx.hypot(dy);
        if dx != 0.0 || dy != 0.0 {
            let angle = dy.atan2(dx);
            if let Some(before) = heading {
                let change = (angle - before).rem_euclid(std::f32::consts::TAU);
                turn += change.min(std::f32::consts::TAU - change);
            }
            heading = Some(angle);
        }
        from = point;
    }
    (length * turn.max(0.05)).sqrt()
}

/// How many segments each curve of `contours` is cut into, in their order: a line and each
/// contour's closing take one; the curves share the rest by their [`weight`], at least one each.
fn steps(contours: &[Contour]) -> Vec<usize> {
    let mut weights = Vec::new();
    let mut lines = contours.len();
    for contour in contours {
        let mut from = contour.start;
        for curve in &contour.curves {
            let end = match *curve {
                Curve::Line(end) | Curve::Quadratic(_, end) | Curve::Cubic(_, _, end) => end,
            };
            match curve {
                Curve::Line(_) => {
                    lines += 1;
                    weights.push(None);
                }
                _ => weights.push(Some(weight(from, curve))),
            }
            from = end;
        }
    }
    let budget = MOST_SEGMENTS - lines;
    let total: f32 = weights.iter().flatten().sum();
    let mut steps: Vec<usize> = weights
        .iter()
        .map(|weight| match weight {
            None => 1,
            Some(weight) => ((budget as f32 * weight / total).floor() as usize).max(1),
        })
        .collect();
    // The floor at one may overspend: take back from the curves cut most.
    while steps.iter().sum::<usize>() + contours.len() > MOST_SEGMENTS {
        let most = (0..steps.len())
            .filter(|&index| weights[index].is_some())
            .max_by_key(|&index| steps[index])
            .expect("a curve");
        steps[most] -= 1;
    }
    steps
}

/// `contours` placed at `center`, `radius` pixels for 1, as at most [`MOST_SEGMENTS`] segments
/// ([`steps`]), each curve cut at even steps of its parameter.
fn flatten(contours: &[Contour], center: Point, radius: f32) -> Vec<[f32; 4]> {
    let mut steps = steps(contours).into_iter();
    let place = |[x, y]: Point| [center[0] + x * radius, center[1] + y * radius];
    let mut segments = Vec::new();
    for contour in contours {
        let mut from = contour.start;
        let mut push = |from: &mut Point, to: Point| {
            let [a, b] = [place(*from), place(to)];
            segments.push([a[0], a[1], b[0], b[1]]);
            *from = to;
        };
        for curve in &contour.curves {
            let start = from;
            let count = steps.next().expect("a count for each curve");
            match *curve {
                Curve::Line(to) => push(&mut from, to),
                Curve::Quadratic(control, end) => {
                    for step in 1..=count {
                        let t = step as f32 / count as f32;
                        let u = 1.0 - t;
                        let at =
                            |i: usize| u * u * start[i] + 2.0 * u * t * control[i] + t * t * end[i];
                        push(&mut from, [at(0), at(1)]);
                    }
                }
                Curve::Cubic(c1, c2, end) => {
                    for step in 1..=count {
                        let t = step as f32 / count as f32;
                        let u = 1.0 - t;
                        let at = |i: usize| {
                            u * u * u * start[i]
                                + 3.0 * u * u * t * c1[i]
                                + 3.0 * u * t * t * c2[i]
                                + t * t * t * end[i]
                        };
                        push(&mut from, [at(0), at(1)]);
                    }
                }
            }
        }
        push(&mut from, contour.start);
    }
    assert!(
        segments.len() <= MOST_SEGMENTS,
        "{} segments",
        segments.len()
    );
    segments
}

/// The path's scene at `w` × `h`: a pale background, then the twenty shapes in five columns and
/// four rows, each in its colour; the last row's shapes clipped to the top two thirds of their row.
pub fn path_screen(canvas: &mut Canvas, w: f32, h: f32) {
    canvas.draw_box(&CanvasBox {
        rect: PixelRect::new(0.0, 0.0, w, h),
        radii: [0.0; 4],
        fill: LinearColor::rgb(0.92, 0.92, 0.95),
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    });
    let (cell_w, cell_h) = (w / 5.0, h / 4.0);
    let radius = 0.4 * cell_w.min(cell_h);
    for (index, contours) in shapes().iter().enumerate() {
        let (column, row) = (index % 5, index / 5);
        let center = [(column as f32 + 0.5) * cell_w, (row as f32 + 0.5) * cell_h];
        let hue = index as f32 / 20.0 * std::f32::consts::TAU;
        let color = LinearColor::rgba(
            0.5 + 0.45 * hue.cos(),
            0.5 + 0.45 * (hue + 2.1).cos(),
            0.5 + 0.45 * (hue + 4.2).cos(),
            if index % 3 == 2 { 0.7 } else { 1.0 },
        );
        let clip =
            (row == 3).then(|| PixelRect::new(0.0, row as f32 * cell_h, w, cell_h * 2.0 / 3.0));
        canvas.draw_path(&flatten(contours, center, radius), color, clip);
    }
}

/// The segments of each shape: their count, for the limits' table.
pub fn segment_counts() -> Vec<usize> {
    shapes()
        .iter()
        .map(|contours| flatten(contours, [0.0, 0.0], 1.0).len())
        .collect()
}
