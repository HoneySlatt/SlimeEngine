//! The effects' scenes, made from the game's menu of `render.canvas` (`canvas::game_screen`): its
//! panels turned, its right panel drawn apart, its panels made of glass; and the growing screen.

use crate::canvas::{
    Canvas, CanvasBox, GAME_PANELS, GameLayout, ImageId, ImageKind, ImagePart, ImageQuad, Item,
    LinearColor, PixelRect, SHADOW, game_cards, game_screen, outward, rotation_about, scale_about,
};

/// The turn of each of the game's panels in the transform's scene, in degrees.
pub const TURNS: [f32; 4] = [-3.0, 2.5, -2.0, 4.0];
/// The card scaled in the transform's scene, and by how much.
pub const SCALED_CARD: usize = 5;
pub const CARD_SCALE: f32 = 1.25;

fn center(rect: PixelRect) -> [f32; 2] {
    [rect.x + rect.width / 2.0, rect.y + rect.height / 2.0]
}

/// Whether `item` belongs to `region`: its shape's centre lies inside it and its shape is no more
/// than a tenth larger, so that a panel holds its box, its shadow and its contents, and not the
/// screen's dimming box.
fn belongs(item: &Item, region: PixelRect) -> bool {
    let [left, top, right, bottom] = item.shape;
    let [x, y] = [(left + right) / 2.0, (top + bottom) / 2.0];
    x >= region.x
        && x < region.x + region.width
        && y >= region.y
        && y < region.y + region.height
        && right - left <= region.width * 1.1
        && bottom - top <= region.height * 1.1
}

/// The game's menu, each panel and its contents turned by [`TURNS`] about the panel's centre and
/// one card scaled by [`CARD_SCALE`] about its own. Quads are kept whole: clips stay in the
/// target's pixels, which a turned item may cross.
pub fn transform_screen(glyphs: ImageId, icons: ImageId, w: f32, h: f32) -> Canvas {
    let mut canvas = Canvas {
        keep_whole: true,
        ..Canvas::default()
    };
    game_screen(&mut canvas, glyphs, icons, w, h);
    let layout = GameLayout::new(w, h);
    let mut regions = Vec::new();
    for ((x, y, width, height), turn) in GAME_PANELS.into_iter().zip(TURNS) {
        let rect = layout.rect(x, y, width, height);
        regions.push((
            rect,
            canvas.add_transform(rotation_about(center(rect), turn)),
        ));
    }
    let (x, y, width, height) = game_cards().nth(SCALED_CARD).expect("16 cards");
    let card = layout.rect(x, y, width, height);
    regions.push((
        card,
        canvas.add_transform(scale_about(center(card), CARD_SCALE)),
    ));
    for item in &mut canvas.items {
        if let Some((_, transform)) = regions.iter().find(|(rect, _)| belongs(item, *rect)) {
            item.kind[1] = *transform;
        }
    }
    canvas
}

/// The group's scene: the game's menu without its right panel's box, rows and text, and those
/// items moved into the group's own texture.
pub struct GroupScene {
    /// The screen, the group's image last, faded to [`GROUP_ALPHA`] and scaled by
    /// [`GROUP_SCALE`] about its centre.
    pub screen: Canvas,
    /// The group's items, their origin the top left corner of [`GroupScene::place`].
    pub group: Canvas,
    /// Where the group's texture lies on the screen: whole pixels.
    pub place: PixelRect,
}

pub const GROUP_ALPHA: f32 = 0.6;
pub const GROUP_SCALE: f32 = 0.95;

impl GroupScene {
    /// The size of the group's texture.
    pub fn size(&self) -> (u32, u32) {
        (self.place.width as u32, self.place.height as u32)
    }
}

/// The right panel's place on the screen at `w` × `h`: its box grown to whole pixels.
pub fn group_place(w: f32, h: f32) -> PixelRect {
    let (x, y, width, height) = GAME_PANELS[3];
    let [left, top, right, bottom] =
        outward(GameLayout::new(w, h).rect(x, y, width, height).edges());
    PixelRect::new(left, top, right - left, bottom - top)
}

/// The group's scene at `w` × `h`; `layer` names the group's texture. The panel's shadow stays
/// on the screen: it lies outside the group's texture.
pub fn group_scene(glyphs: ImageId, icons: ImageId, layer: ImageId, w: f32, h: f32) -> GroupScene {
    let mut full = Canvas::default();
    game_screen(&mut full, glyphs, icons, w, h);
    let (x, y, width, height) = GAME_PANELS[3];
    let panel = GameLayout::new(w, h).rect(x, y, width, height);
    let place = group_place(w, h);
    let mut screen = Canvas::default();
    let mut group = Canvas::default();
    for (item, &image) in full.items.iter().zip(&full.item_images) {
        if item.kind[0] != SHADOW && belongs(item, panel) {
            group.items.push(item.translated(-place.x, -place.y));
            group.item_images.push(image);
        } else {
            screen.items.push(*item);
            screen.item_images.push(image);
        }
    }
    screen.draw_image(&ImageQuad {
        rect: place,
        image: layer,
        part: ImagePart::WHOLE,
        kind: ImageKind::Layer,
        color: LinearColor::rgba(1.0, 1.0, 1.0, GROUP_ALPHA),
        clip: None,
    });
    let scaled = screen.add_transform(scale_about(center(place), GROUP_SCALE));
    screen.items.last_mut().expect("the group's image").kind[1] = scaled;
    GroupScene {
        screen,
        group,
        place,
    }
}

/// The blur's radius at a screen `h` pixels high: 24 at 1080, scaled with the screen.
pub fn blur_radius(h: f32) -> f32 {
    24.0 * h / 1080.0
}

/// The game's four panels, the glasses, at `w` × `h`.
pub fn glass_panels(w: f32, h: f32) -> [PixelRect; 4] {
    let layout = GameLayout::new(w, h);
    GAME_PANELS.map(|(x, y, width, height)| layout.rect(x, y, width, height))
}

/// The tint of a glass: the panel's fill, lighter.
const GLASS_TINT: LinearColor = LinearColor::rgba(0.08, 0.08, 0.1, 0.45);

/// The blur's scene: `behind`, what lies behind the glasses, a game's view (the 16 pictures
/// across the screen, a few bright boxes, rows of text) under the menu's dimming box; `front`,
/// the rest of the menu, its four panels' boxes made glasses that read `backdrops`, each the image
/// of the region it names.
pub fn blur_scene(
    glyphs: ImageId,
    icons: ImageId,
    pictures: &[ImageId],
    backdrops: &[(ImageId, PixelRect); 4],
    w: f32,
    h: f32,
) -> (Canvas, Canvas) {
    let mut menu = Canvas::default();
    game_screen(&mut menu, glyphs, icons, w, h);
    let scale = GameLayout::new(w, h).scale;
    let mut behind = Canvas::default();
    let (tile_w, tile_h) = (w / 4.0, h / 4.0);
    for (index, &picture) in pictures.iter().enumerate() {
        behind.draw_image(&ImageQuad {
            rect: PixelRect::new(
                (index % 4) as f32 * tile_w,
                (index / 4) as f32 * tile_h,
                tile_w,
                tile_h,
            ),
            image: picture,
            part: ImagePart::WHOLE,
            kind: ImageKind::Color,
            color: LinearColor::WHITE,
            clip: None,
        });
    }
    for index in 0..12 {
        let size = (60.0 + 25.0 * (index % 5) as f32) * scale;
        behind.draw_box(&CanvasBox {
            rect: PixelRect::new(
                (index as f32 * 0.37 % 1.0) * (w - size),
                (index as f32 * 0.61 % 1.0) * (h - size),
                size,
                size,
            ),
            radii: [size / 2.0; 4],
            fill: LinearColor::rgb(1.0, 0.8 - 0.05 * index as f32, 0.1 + 0.07 * index as f32),
            border_width: 0.0,
            border: LinearColor::BLACK,
            clip: None,
        });
    }
    for line in 0..20 {
        for column in 0..(w / (10.0 * scale)) as usize {
            let cell = line * 64 + column;
            behind.draw_image(&ImageQuad {
                rect: PixelRect::new(
                    column as f32 * 10.0 * scale,
                    line as f32 * h / 20.0,
                    10.0 * scale,
                    18.0 * scale,
                ),
                image: glyphs,
                part: ImagePart {
                    x: (cell % 64) as f32 / 64.0,
                    y: (cell / 64 % 64) as f32 / 64.0,
                    width: 1.0 / 64.0,
                    height: 1.0 / 64.0,
                },
                kind: ImageKind::Coverage,
                color: LinearColor::WHITE,
                clip: None,
            });
        }
    }
    // The menu's dimming box lies behind its panels too.
    behind.items.push(menu.items[0]);
    behind.item_images.push(menu.item_images[0]);

    let panels = glass_panels(w, h);
    let mut front = Canvas::default();
    for (item, &image) in menu.items.iter().zip(&menu.item_images).skip(1) {
        let glass = panels
            .iter()
            .position(|panel| item.kind[0] == crate::canvas::BOX && item.shape == panel.edges());
        match glass {
            Some(index) => front.draw_glass(
                &CanvasBox {
                    rect: panels[index],
                    radii: [12.0 * scale; 4],
                    fill: GLASS_TINT,
                    border_width: 2.0 * scale,
                    border: LinearColor::rgb(0.5, 0.42, 0.3),
                    clip: None,
                },
                backdrops[index].0,
                backdrops[index].1,
            ),
            None => {
                front.items.push(*item);
                front.item_images.push(image);
            }
        }
    }
    (behind, front)
}

/// The growing screen's sizes: its items.
pub const GROWING: [usize; 3] = [1_000, 10_000, 100_000];

/// A still screen of `count` items at `w` × `h`, an opaque background then rows, each a box and
/// glyphs, the glyphs as small as fill the screen, and in the middle of the rows a box that moves:
/// its place at frame `frame` ([`moving_box`]). Gives the moving box's index.
pub fn growing_screen(
    canvas: &mut Canvas,
    glyphs: ImageId,
    count: usize,
    w: f32,
    h: f32,
    frame: usize,
) -> usize {
    canvas.draw_box(&CanvasBox {
        rect: PixelRect::new(0.0, 0.0, w, h),
        radii: [0.0; 4],
        fill: LinearColor::rgb(0.06, 0.06, 0.07),
        border_width: 0.0,
        border: LinearColor::BLACK,
        clip: None,
    });
    let glyph_w = (w * h / (1.75 * count as f32)).sqrt();
    let glyph_h = 1.75 * glyph_w;
    let per_row = ((w / glyph_w) as usize).max(1);
    let rows = (count - 2).div_ceil(per_row + 1);
    let mut moving = 0;
    let mut row = 0;
    while canvas.items.len() < count {
        if row == rows / 2 && moving == 0 {
            moving = canvas.items.len();
            canvas.draw_box(&moving_box(w, h, frame));
            continue;
        }
        let y = (row as f32 * glyph_h) % h;
        canvas.draw_box(&CanvasBox {
            rect: PixelRect::new(0.0, y, w, glyph_h),
            radii: [glyph_h * 0.2; 4],
            fill: if row % 2 == 0 {
                LinearColor::rgb(0.11, 0.11, 0.13)
            } else {
                LinearColor::rgb(0.09, 0.09, 0.1)
            },
            border_width: 0.0,
            border: LinearColor::BLACK,
            clip: None,
        });
        for column in 0..per_row.min(count.saturating_sub(canvas.items.len())) {
            let cell = (row * per_row + column) % 4096;
            canvas.draw_image(&ImageQuad {
                rect: PixelRect::new(column as f32 * glyph_w, y, glyph_w, glyph_h),
                image: glyphs,
                part: ImagePart {
                    x: (cell % 64) as f32 / 64.0,
                    y: (cell / 64) as f32 / 64.0,
                    width: 1.0 / 64.0,
                    height: 1.0 / 64.0,
                },
                kind: ImageKind::Coverage,
                color: LinearColor::rgb(0.85, 0.85, 0.8),
                clip: None,
            });
        }
        row += 1;
    }
    moving
}

/// The moving box at frame `frame`: 48 pixels at 1080, sliding right 7 pixels a frame across a
/// line at two fifths of the screen's height, back to the left at the edge.
pub fn moving_box(w: f32, h: f32, frame: usize) -> CanvasBox {
    let scale = h / 1080.0;
    let size = 48.0 * scale;
    let x = (frame as f32 * 7.0 * scale) % (w - size);
    CanvasBox {
        rect: PixelRect::new(x, 0.4 * h, size, size),
        radii: [8.0 * scale; 4],
        fill: LinearColor::rgb(0.95, 0.55, 0.1),
        border_width: 2.0 * scale,
        border: LinearColor::WHITE,
        clip: None,
    }
}

/// Moves the box `index` of `canvas` to its place at frame `frame`, made again alone.
pub fn move_box(canvas: &mut Canvas, index: usize, w: f32, h: f32, frame: usize) {
    let mut alone = Canvas::default();
    alone.draw_box(&moving_box(w, h, frame));
    canvas.items[index] = alone.items[0];
}
