//! An interface drawn by the engine over a window or into a PNG file: a tree of elements, laid out
//! by flexbox and grid (`docs/adr/0140-the-tree-of-elements.md`) and styled by a style sheet and a
//! theme (`docs/adr/0141-style-sheets-and-themes.md`), gives the display list that
//! `slime-ui-render` draws through the renderer's canvas (`docs/adr/0137-the-canvas.md`): panels
//! with rounded corners, borders and shadows; a list of 100 000 rows that scrolls with time, of
//! which only those that show are built, cut by its panel; buttons with their labels, one shown
//! pressed; a sentence of each writing system of slime-ui's tests, Arabic and Hebrew from right to
//! left, Hindi, Thai, Japanese, Chinese and emoji, its glyphs kept in the bridge's atlas; images
//! of colors, and images of coverage drawn in a color; and, in a panel, the shapes of the image
//! tests, which the renderer draws into a target of its own each frame, the panel showing that
//! target as an image, as the editor's viewport will, a translucent card over it in the same cell
//! of a grid. Colors mix as sRGB values, as design tools mix them. The fonts are those of
//! slime-ui's tests, parts of Noto fonts that hold the characters of its texts alone
//! (`crates/slime-ui/tests/fonts/README.md`), built into the program.
//!
//! The code gives the elements their layout and their classes; the style sheet
//! `examples/assets/ui/interface.style` gives the rest, with the values of a dark or a light theme,
//! which T switches. They are read cooked, as the sample's scenes are:
//!
//! ```sh
//! cargo run -p slime-cook -- cook examples/assets target/cooked/examples
//! cargo run --example interface                            # in a window; T switches the theme
//! cargo run --example interface -- --frames 120            # closes itself after 120 frames
//! cargo run --example interface -- --output interface.png  # one image of 1280 × 800, no window
//! cargo run --example interface -- --output interface@2.png --scale 2
//! cargo run --example interface -- --theme light           # the light theme first
//! cargo run --example interface -- --right-to-left         # the layout mirrored, from the right
//! cargo run --example interface -- --watch                 # styled again as the cooked data changes
//! ```
//!
//! With `--watch`, beside `cargo run --release -p slime-cook -- watch examples/assets
//! target/cooked/examples`, a change to the style sheet or a theme restyles the running example.
//! Without `--assets`, it reads `assets` beside the program if there is one, as where CI ships it,
//! and `target/cooked/examples` otherwise.

#[path = "common/assets_directory.rs"]
mod assets_directory;
#[path = "common/png_file.rs"]
mod png_file;
#[path = "common/shapes.rs"]
mod shapes;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use slime_app::{AppConfig, AppError, Context, Presentation};
use slime_assets::{AssetId, CookedAssets, ManifestWatch};
use slime_color::LinearColor;
use slime_gfx::{Gpu, RenderTarget, Texture, TextureFormat};
use slime_platform::{ButtonState, Event, KeyCode, KeyEvent, WindowConfig};
use slime_render::{CanvasImage, RenderError};
use slime_ui::Rect;
use slime_ui::display::{ImageKey, ImageKind};
use slime_ui::style::{Align, Dimension, Display, FlexDirection, Sides, Style, Track};
use slime_ui::text::Fonts;
use slime_ui::theme::{StyleSheet, Theme};
use slime_ui::tree::{
    Children, Element, ElementId, ElementState, Kind, LayoutDirection, RootSettings, Ui,
};
use slime_ui_render::UiRenderer;
use slime_world::World;

use shapes::Shapes;

const USAGE: &str = "usage: interface [--assets <directory>] [--theme dark|light] [--right-to-left] \
                     [[--frames <count>] [--watch] | --output <file.png> [--scale <factor>]]";
/// The size of the image that `--output` writes, in logical pixels, and the moment it shows.
const IMAGE_SIZE: (u32, u32) = (1280, 800);
const IMAGE_TIME: f32 = 0.7;
/// The side of an icon of the atlas of colors, and of the image of coverage, in texels.
const ICON: u32 = 32;
const MASK: u32 = 64;
/// The keys by which the display list names its images.
const ICONS: ImageKey = ImageKey(1);
const TICK: ImageKey = ImageKey(2);
const VIEW: ImageKey = ImageKey(3);
/// The width of the panel of writing systems, in logical pixels.
const TEXTS_WIDTH: f32 = 360.0;
/// The IDs of the style sheet and the themes, in their metadata files in `examples/assets/ui`.
const STYLE: AssetId = AssetId::from_u128(0xa3a6260c_7bc3_4e47_aa2b_d700551d3749);
const DARK: AssetId = AssetId::from_u128(0x4d72297f_3eb0_4a03_87d3_3f75543e0662);
const LIGHT: AssetId = AssetId::from_u128(0xb882b333_a611_4f74_adc7_d0a845e5cdf1);

/// The fonts of slime-ui's tests, built into the program so that it runs anywhere.
const FONTS: &[&[u8]] = &[
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSans.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansArabic.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansHebrew.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansDevanagari.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansThai.ttf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansCJKjp.otf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoSansCJKsc.otf"),
    include_bytes!("../crates/slime-ui/tests/fonts/NotoColorEmoji.ttf"),
];

/// Each writing system: the first sentence of its test text, its stack and its language.
const SCRIPTS: &[(&str, [&str; 2], &str)] = &[
    (
        include_str!("../crates/slime-ui/tests/texts/english.txt"),
        ["Noto Sans", "Noto Sans"],
        "en",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/arabic.txt"),
        ["Noto Sans Arabic", "Noto Sans"],
        "ar",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/hebrew.txt"),
        ["Noto Sans Hebrew", "Noto Sans"],
        "he",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/hindi.txt"),
        ["Noto Sans Devanagari", "Noto Sans"],
        "hi",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/thai.txt"),
        ["Noto Sans Thai", "Noto Sans"],
        "th",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/japanese.txt"),
        ["Noto Sans CJK JP", "Noto Sans"],
        "ja",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/chinese.txt"),
        ["Noto Sans CJK SC", "Noto Sans"],
        "zh-CN",
    ),
    (
        include_str!("../crates/slime-ui/tests/texts/emoji.txt"),
        ["Noto Sans", "Noto Color Emoji"],
        "en",
    ),
];

/// What the interface draws with: the renderer, which the shapes hold, the target their view is
/// drawn into, the images, the tree of elements with its fonts, and the bridge with its atlas.
struct Interface {
    shapes: Shapes,
    viewport: Option<RenderTarget>,
    /// Sixteen icons of 32 × 32 texels in colors, four by four.
    icons: Texture,
    /// A ring and a tick, how much of each texel they cover.
    tick: Texture,
    ui: Ui,
    root: ElementId,
    direction: LayoutDirection,
    renderer: UiRenderer,
    /// The words that name the list's rows and the buttons, those of the English test text, which
    /// its fonts hold.
    words: Vec<String>,
    /// The elements that the program reads or changes: the list, which scrolls with time; the
    /// image of the view, whose box sizes the view's target; the button shown pressed.
    list: Option<ElementId>,
    view: Option<ElementId>,
    pressed: Option<ElementId>,
    /// The cooked data that holds the style sheet and the themes, and the theme shown.
    assets: CookedAssets,
    theme: ThemeName,
}

/// The themes of the example.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ThemeName {
    #[default]
    Dark,
    Light,
}

impl Interface {
    fn new(
        gpu: &Gpu,
        format: TextureFormat,
        direction: LayoutDirection,
        assets: CookedAssets,
        theme: ThemeName,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let side = ICON * 4;
        let icons: Vec<u8> = (0..side * side)
            .flat_map(|texel| {
                let (x, y) = (texel % side, texel / side);
                let (column, row) = (x / ICON, y / ICON);
                let (u, v) = ((x % ICON) as f32 + 0.5, (y % ICON) as f32 + 0.5);
                // A disc of each icon's color, fading to its edge, on its color, transparent, so
                // that filtering at its edge darkens nothing.
                let distance = ((u - 16.0).powi(2) + (v - 16.0).powi(2)).sqrt();
                let alpha = (14.0 - distance).clamp(0.0, 1.0);
                [
                    (64 + column * 60) as u8,
                    (64 + row * 60) as u8,
                    (240 - column * 40) as u8,
                    (alpha * 255.0) as u8,
                ]
            })
            .collect();
        let icons =
            gpu.create_texture("icons", side, side, TextureFormat::Rgba8UnormSrgb, &icons)?;
        let tick: Vec<u8> = (0..MASK * MASK)
            .map(|texel| {
                let (x, y) = ((texel % MASK) as f32 + 0.5, (texel / MASK) as f32 + 0.5);
                let ring = ((x - 32.0).powi(2) + (y - 32.0).powi(2)).sqrt();
                let ring = 1.0 - ((ring - 26.0).abs() - 3.0).clamp(0.0, 1.0);
                // The tick: two strokes, from (18, 33) to (28, 43), then to (46, 22).
                let stroke = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| {
                    let (dx, dy) = (bx - ax, by - ay);
                    let along =
                        (((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
                    let (px, py) = (ax + along * dx - x, ay + along * dy - y);
                    1.0 - ((px * px + py * py).sqrt() - 3.0).clamp(0.0, 1.0)
                };
                let tick =
                    stroke((18.0, 33.0), (28.0, 43.0)).max(stroke((28.0, 43.0), (46.0, 22.0)));
                (ring.max(tick) * 255.0) as u8
            })
            .collect();
        let tick =
            gpu.create_texture_with_levels("tick", MASK, MASK, TextureFormat::R8Unorm, &[&tick])?;
        let mut fonts = Fonts::new();
        for font in FONTS {
            fonts.register(font.to_vec())?;
        }
        let words = SCRIPTS[0]
            .0
            .split_whitespace()
            .map(|word| word.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|word| word.len() > 3)
            .map(str::to_owned)
            .collect();
        let mut ui = Ui::new(fonts);
        let settings = RootSettings {
            size: [1.0, 1.0],
            scale: 1.0,
            direction,
        };
        let root = ui.add_root(settings, column());
        let mut interface = Self {
            shapes: Shapes::new(gpu, format)?,
            viewport: None,
            icons,
            tick,
            ui,
            root,
            direction,
            renderer: UiRenderer::default(),
            words,
            list: None,
            view: None,
            pressed: None,
            assets,
            theme,
        };
        interface.style()?;
        Ok(interface)
    }

    /// Styles the interface with its style sheet and its theme, as its cooked data holds them.
    ///
    /// # Errors
    ///
    /// If the cooked data lacks them, or they cannot style it: it keeps the style it had.
    fn style(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let theme = match self.theme {
            ThemeName::Dark => DARK,
            ThemeName::Light => LIGHT,
        };
        // Named in errors by their sources' paths.
        let name = |id| {
            self.assets
                .manifest()
                .get(id)
                .map_or_else(|| id.to_string(), |entry| entry.source.clone())
        };
        let sheet = StyleSheet::from_binary(name(STYLE), self.assets.style_sheet(STYLE)?);
        let theme = Theme::from_binary(name(theme), self.assets.theme(theme)?);
        self.ui.set_style(&[sheet], &theme)?;
        Ok(())
    }

    /// Draws the interface over `target`, `time` seconds after the start, at `scale` pixels per
    /// logical pixel: builds its elements when they change, lays them out, draws the shapes into
    /// the view's target at the size of its image, then the display list.
    fn draw(
        &mut self,
        gpu: &Gpu,
        target: &RenderTarget,
        time: f32,
        scale: f32,
    ) -> Result<(), RenderError> {
        let size = [
            target.width() as f32 / scale,
            target.height() as f32 / scale,
        ];
        self.ui.set_root(
            self.root,
            RootSettings {
                size,
                scale,
                direction: self.direction,
            },
        );
        if let Some(list) = self.list {
            self.ui.scroll_to(list, [0.0, time * 24.0]);
        }
        // Nothing that the builders read changes: the interface's runs at the first frame, the
        // list's, in its slot, then again when the list shows other rows.
        let (mut list, mut view, mut pressed) = (None, None, None);
        let words = &self.words;
        self.ui.build(self.root, 0, |children| {
            build(children, words, &mut list, &mut view, &mut pressed);
        });
        self.list = list.or(self.list);
        if let Some(list) = self.list {
            self.ui.build(list, 0, |rows| {
                for row in rows.rows() {
                    list_row(rows, words, row);
                }
            });
        }
        self.view = view.or(self.view);
        self.pressed = pressed.or(self.pressed);
        if let Some(pressed) = self.pressed {
            self.ui.set_state(
                pressed,
                ElementState {
                    pressed: true,
                    ..ElementState::default()
                },
            );
        }
        let (display, _) = self.ui.frame();

        // The view, drawn into its own target at the size of its image.
        let inside = self
            .view
            .and_then(|view| self.ui.rect(view))
            .unwrap_or_default();
        let size = (inside.width.max(1.0) as u32, inside.height.max(1.0) as u32);
        if self
            .viewport
            .as_ref()
            .is_none_or(|viewport| (viewport.width(), viewport.height()) != size)
        {
            self.viewport =
                Some(gpu.create_render_target("viewport", size.0, size.1, target.format())?);
        }
        let viewport = self.viewport.as_ref().expect("made above");
        self.shapes.render(gpu, viewport, time)?;
        let (icons, tick) = (&self.icons, &self.tick);
        self.renderer.draw(
            gpu,
            self.shapes.renderer(),
            self.ui.fonts(),
            &display,
            |key| match key {
                ICONS => Some(CanvasImage::Texture(icons)),
                TICK => Some(CanvasImage::Texture(tick)),
                VIEW => Some(CanvasImage::Target(viewport)),
                _ => None,
            },
            target,
        )?;
        Ok(())
    }
}

/// Builds the interface: a bar of buttons, the second shown pressed; a list of 100 000 rows in a
/// slot, whose rows its own builder gives; the view, a translucent card over it in the same cell of
/// a grid; a shelf of cards, cut where the window ends; and a sentence of each writing system. The
/// elements' classes name what the style sheet styles; their own style lays them out.
fn build(
    children: &mut Children<'_>,
    words: &[String],
    list: &mut Option<ElementId>,
    view: &mut Option<ElementId>,
    pressed: &mut Option<ElementId>,
) {
    let mut window = column();
    window.layout.grow = 1.0;
    window.layout.min_size[1] = Dimension::Pixels(0.0);
    let window = Element::container(window).class("window");
    children.add("window", window, |children| {
        let mut bar = Style::default();
        bar.layout.size[1] = Dimension::Pixels(40.0);
        bar.layout.padding = Sides::symmetric(12.0, 6.0);
        bar.layout.gap = [0.0, 8.0];
        children.add("bar", Element::container(bar).class("bar"), |children| {
            for index in 0..5_usize {
                let element = Element::button(words[index * 7].as_str(), sized(88.0, 28.0));
                let id = children.add(index, element, |_| {});
                if index == 1 {
                    *pressed = Some(id);
                }
            }
        });
        let mut main = Style::default();
        main.layout.grow = 1.0;
        main.layout.min_size[1] = Dimension::Pixels(0.0);
        main.layout.padding = Sides::all(12.0);
        main.layout.gap = [0.0, 12.0];
        children.add("main", Element::container(main), |children| {
            let mut rows = Style::default();
            rows.layout.size[0] = Dimension::Pixels(260.0);
            rows.layout.padding = Sides::all(8.0);
            let panel = Element::container(rows).class("panel");
            children.add("list panel", panel, |children| {
                let mut style = Style::default();
                style.layout.grow = 1.0;
                let element = Element::list(100_000, 28.0, style);
                *list = Some(children.add_slot("list", element));
            });
            let mut middle = column();
            middle.layout.grow = 1.0;
            middle.layout.min_size = [Dimension::Pixels(0.0); 2];
            middle.layout.gap = [12.0, 0.0];
            children.add("middle", Element::container(middle), |children| {
                let mut cell = Style::default();
                cell.layout.grow = 1.0;
                cell.layout.display = Display::Grid;
                cell.layout.columns = vec![Track::Fraction(1.0)];
                cell.layout.rows = vec![Track::Fraction(1.0)];
                cell.layout.padding = Sides::all(6.0);
                let panel = Element::container(cell).class("panel");
                children.add("view panel", panel, |children| {
                    let mut image = Style::default();
                    image.layout.column.start = Some(1);
                    image.layout.row.start = Some(1);
                    let element = Element::image(VIEW, image.clone());
                    *view = Some(children.add("view", element, |_| {}));
                    card(children, words, image);
                });
                let mut shelf = Style::default();
                shelf.layout.size[1] = Dimension::Pixels(128.0);
                shelf.layout.gap = [0.0, 12.0];
                children.add("shelf", Element::scroll(shelf), |children| {
                    for index in 0..8_usize {
                        shelf_card(children, index);
                    }
                });
            });
            let mut texts = Style::default();
            texts.layout.size[0] = Dimension::Pixels(TEXTS_WIDTH);
            texts.layout.padding = Sides::all(12.0);
            texts.layout.direction = FlexDirection::Column;
            texts.layout.gap = [10.0, 0.0];
            let panel = Element::container(texts).class("panel").class("texts");
            children.add("texts", panel, |children| {
                // A sentence of each writing system, across the panel, from its start: from the
                // right for Arabic and Hebrew. Its fonts and language are its own; its size and
                // color, the style sheet's.
                for (index, &(script, stack, locale)) in SCRIPTS.iter().enumerate() {
                    let mut style = Style::default();
                    style.text.style.fonts =
                        stack.iter().map(|&family| family.to_owned()).collect();
                    style.text.style.locale = Some(locale.to_owned());
                    let sentence = script.lines().next().unwrap_or_default();
                    children.add(index, Element::label(sentence, style), |_| {});
                }
            });
        });
    });
}

/// A row of the list: an icon and a name in a box, the sixth selected.
fn list_row(rows: &mut Children<'_>, words: &[String], row: usize) {
    let mut space = Style::default();
    space.layout.padding.bottom = 4.0;
    rows.add(row, Element::container(space), |children| {
        let mut line = Style::default();
        line.layout.grow = 1.0;
        line.layout.padding.start = 6.0;
        line.layout.gap = [0.0, 8.0];
        line.layout.align_items = Align::Center;
        let mut line = Element::container(line).class("row");
        if row % 2 == 1 {
            line = line.class("odd");
        }
        if row == 5 {
            line = line.class("selected");
        }
        children.add("line", line, |children| {
            children.add("icon", icon(row % 16, 16.0), |_| {});
            let word = words[row % words.len()].as_str();
            children.add("name", Element::label(word, Style::default()), |_| {});
        });
    });
}

/// The translucent card, at the top start of the view's cell, over the view.
fn card(children: &mut Children<'_>, words: &[String], mut style: Style) {
    style.layout.size = [Dimension::Pixels(220.0), Dimension::Pixels(96.0)];
    style.layout.align_self = Some(Align::Start);
    style.layout.margin = Sides::all(16.0);
    style.layout.padding = Sides::all(16.0);
    style.layout.gap = [0.0, 12.0];
    style.layout.align_items = Align::Center;
    children.add(
        "card",
        Element::container(style).class("card"),
        |children| {
            children.add("tick", tick(srgb(0x51CF66), 64.0), |_| {});
            let caption = Element::label(words[3].as_str(), Style::default());
            children.add("caption", caption, |_| {});
        },
    );
}

/// A card of the shelf: an icon, and a tick in a color.
fn shelf_card(children: &mut Children<'_>, index: usize) {
    let mut style = sized(100.0, 128.0);
    style.layout.direction = FlexDirection::Column;
    style.layout.align_items = Align::Center;
    style.layout.padding.top = 12.0;
    style.layout.gap = [8.0, 0.0];
    let tint = [
        srgb(0xFF6B6B),
        srgb(0xFCC419),
        srgb(0x51CF66),
        srgb(0x339AF0),
    ][index % 4];
    children.add(
        index,
        Element::container(style).class("panel"),
        |children| {
            children.add("icon", icon((index * 5) % 16, 64.0), |_| {});
            children.add("tick", tick(tint, 32.0), |_| {});
        },
    );
}

/// The icon `icon` of the atlas of colors, `side` logical pixels a side.
fn icon(icon: usize, side: f32) -> Element {
    let kind = Kind::Image {
        image: ICONS,
        part: Rect::new((icon % 4) as f32 / 4.0, (icon / 4) as f32 / 4.0, 0.25, 0.25),
        kind: ImageKind::Color,
        color: LinearColor::WHITE,
    };
    Element::of(kind, sized(side, side))
}

/// The ring and the tick, drawn in `color`, `side` logical pixels a side.
fn tick(color: LinearColor, side: f32) -> Element {
    let kind = Kind::Image {
        image: TICK,
        part: Rect::new(0.0, 0.0, 1.0, 1.0),
        kind: ImageKind::Coverage,
        color,
    };
    Element::of(kind, sized(side, side))
}

/// A style that lays its children out in a column.
fn column() -> Style {
    let mut style = Style::default();
    style.layout.direction = FlexDirection::Column;
    style
}

/// A style of `width` by `height` logical pixels, which does not shrink.
fn sized(width: f32, height: f32) -> Style {
    let mut style = Style::default();
    style.layout.size = [Dimension::Pixels(width), Dimension::Pixels(height)];
    style.layout.shrink = 0.0;
    style
}

/// A color given as sRGB's 0xRRGGBB.
fn srgb(hex: u32) -> LinearColor {
    LinearColor::from_srgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

#[derive(Default)]
struct InterfaceExample {
    interface: Option<Interface>,
    /// The cooked data, until the interface takes it.
    assets: Option<CookedAssets>,
    theme: ThemeName,
    direction: LayoutDirection,
    /// With `--watch`, what tells when the cooked data changes.
    watch: Option<ManifestWatch>,
    /// How many frames drew the interface.
    drawn: u64,
    /// Whether a run that drew nothing fails, as the automated checks run it.
    must_draw: bool,
}

impl Presentation for InterfaceExample {
    fn start(&mut self, ctx: &mut Context, _world: &World) -> Result<(), AppError> {
        if let (Some(gpu), Some(format), Some(assets)) =
            (ctx.gpu(), ctx.surface_format(), self.assets.take())
        {
            let interface = Interface::new(gpu, format, self.direction, assets, self.theme)?;
            self.interface = Some(interface);
        }
        Ok(())
    }

    fn on_event(&mut self, ctx: &mut Context, event: &Event) -> bool {
        let Event::Key(KeyEvent {
            physical,
            state: ButtonState::Pressed,
            ..
        }) = event
        else {
            return false;
        };
        match physical {
            KeyCode::Escape => ctx.request_exit(),
            KeyCode::KeyT => {
                if let Some(interface) = &mut self.interface {
                    let before = interface.theme;
                    interface.theme = match before {
                        ThemeName::Dark => ThemeName::Light,
                        ThemeName::Light => ThemeName::Dark,
                    };
                    match interface.style() {
                        Ok(()) => tracing::info!(theme = ?interface.theme, "the theme changed"),
                        Err(error) => {
                            interface.theme = before;
                            tracing::warn!("the theme cannot change: {error}");
                        }
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn render(&mut self, ctx: &mut Context, target: &RenderTarget) {
        if let (Some(watch), Some(interface)) = (&mut self.watch, &mut self.interface) {
            match watch.poll(Instant::now()) {
                Ok(Some(assets)) => {
                    interface.assets = assets;
                    match interface.style() {
                        Ok(()) => {
                            tracing::info!("the cooked data changed: the interface is styled again")
                        }
                        Err(error) => tracing::warn!(
                            "the interface cannot be styled again: {error}; it keeps its style"
                        ),
                    }
                }
                Ok(None) => {}
                Err(error) => tracing::warn!(
                    "the cooked data cannot be read: {error}; the interface keeps its style"
                ),
            }
        }
        let scale = ctx
            .window()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let time = ctx.time().real().elapsed().as_secs_f32();
        if let (Some(gpu), Some(interface)) = (ctx.gpu(), &mut self.interface) {
            match interface.draw(gpu, target, time, scale) {
                Ok(()) => self.drawn += 1,
                Err(error) => tracing::warn!("frame not drawn: {error}"),
            }
        }
    }

    fn stop(&mut self, ctx: &mut Context) {
        tracing::info!(drawn = self.drawn, "stop");
        if self.must_draw && self.drawn == 0 {
            ctx.exit_with_error("no frame drew the interface");
        }
    }
}

fn main() -> ExitCode {
    let options = match parse_options() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let directory = options
        .assets
        .clone()
        .unwrap_or_else(assets_directory::find);
    let assets = match CookedAssets::open(&directory) {
        Ok(assets) => assets,
        Err(error) => {
            eprintln!(
                "{error}\nfor the sample: cargo run -p slime-cook -- cook examples/assets \
                 target/cooked/examples"
            );
            return ExitCode::FAILURE;
        }
    };
    if let Some(output) = &options.output {
        slime_app::init_diagnostics();
        return match write_image(output, &options, assets) {
            Ok(()) => {
                tracing::info!(path = %output.display(), "image written");
                ExitCode::SUCCESS
            }
            Err(error) => {
                tracing::error!("{}", slime_app::error_chain(error.as_ref()));
                ExitCode::FAILURE
            }
        };
    }
    let config = AppConfig {
        window: Some(WindowConfig {
            title: "SlimeEngine interface example".to_owned(),
            ..WindowConfig::default()
        }),
        frame_limit: options.frame_limit,
        ..AppConfig::default()
    };
    let mut example = InterfaceExample {
        must_draw: options.frame_limit.is_some_and(|limit| limit > 0),
        direction: options.direction,
        theme: options.theme,
        watch: options
            .watch
            .then(|| ManifestWatch::new(&assets, Instant::now())),
        assets: Some(assets),
        ..InterfaceExample::default()
    };
    match slime_app::run(&mut (), &mut example, &config) {
        Ok(()) => ExitCode::SUCCESS,
        // `run` has logged the error with its causes.
        Err(_) => ExitCode::FAILURE,
    }
}

/// Draws the interface, without a window, into an image written to `output`.
fn write_image(
    output: &Path,
    options: &Options,
    assets: CookedAssets,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let gpu = Gpu::new()?;
    let (width, height) = (
        (IMAGE_SIZE.0 as f32 * options.scale).round() as u32,
        (IMAGE_SIZE.1 as f32 * options.scale).round() as u32,
    );
    let target =
        gpu.create_render_target("interface", width, height, TextureFormat::Rgba8UnormSrgb)?;
    let mut interface = Interface::new(
        &gpu,
        target.format(),
        options.direction,
        assets,
        options.theme,
    )?;
    // The first frame lays the list out, the second builds the rows that it shows.
    interface.draw(&gpu, &target, IMAGE_TIME, options.scale)?;
    interface.draw(&gpu, &target, IMAGE_TIME, options.scale)?;
    let image = gpu.read_render_target(&target)?;
    png_file::write(output, &image)
        .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    Ok(())
}

/// The command-line options of the example.
struct Options {
    assets: Option<PathBuf>,
    frame_limit: Option<u64>,
    output: Option<PathBuf>,
    scale: f32,
    direction: LayoutDirection,
    theme: ThemeName,
    watch: bool,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        assets: None,
        frame_limit: None,
        output: None,
        scale: 1.0,
        direction: LayoutDirection::LeftToRight,
        theme: ThemeName::Dark,
        watch: false,
    };
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--assets") => {
                options.assets = Some(arguments.next().ok_or("--assets needs a directory")?.into());
            }
            Some("--frames") => {
                let value = arguments.next().ok_or("--frames needs a value")?;
                options.frame_limit = Some(
                    value
                        .to_str()
                        .and_then(|value| value.parse().ok())
                        .ok_or_else(|| format!("invalid frame count `{}`", value.display()))?,
                );
            }
            Some("--right-to-left") => options.direction = LayoutDirection::RightToLeft,
            Some("--theme") => {
                options.theme = match arguments.next().as_ref().and_then(|value| value.to_str()) {
                    Some("dark") => ThemeName::Dark,
                    Some("light") => ThemeName::Light,
                    _ => return Err("--theme needs `dark` or `light`".to_owned()),
                };
            }
            Some("--watch") => options.watch = true,
            Some("--output") => {
                options.output = Some(arguments.next().ok_or("--output needs a path")?.into());
            }
            Some("--scale") => {
                options.scale = arguments
                    .next()
                    .and_then(|value| value.to_str()?.parse::<f32>().ok())
                    .filter(|&scale| scale > 0.0 && scale <= 4.0)
                    .ok_or("--scale needs a factor above 0 and up to 4")?;
            }
            _ => return Err(format!("unknown argument `{}`", argument.display())),
        }
    }
    if options.output.is_some() && (options.frame_limit.is_some() || options.watch) {
        return Err("--frames and --watch draw in a window, --output into a file".to_owned());
    }
    if options.output.is_none() && options.scale != 1.0 {
        return Err("--scale goes with --output; a window has its own scale".to_owned());
    }
    Ok(options)
}
