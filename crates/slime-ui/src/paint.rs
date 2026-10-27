//! What each element draws, kept until it changes, and the display list gathered from it.

use crate::Rect;
use crate::display::{
    BoxPrimitive, DisplayList, ImagePrimitive, Primitive, ShadowPrimitive, TextPrimitive, snap,
};
use crate::style::{Display, PaintStyle};
use crate::text::TextBounds;
use crate::tree::{ElementId, Kind, LayoutDirection, Ui};

/// Draws again the elements of `root` that changed since they were last drawn, and gives how
/// many.
pub(crate) fn paint(ui: &mut Ui, root: ElementId) -> usize {
    let Some(settings) = ui.node(root).and_then(|node| node.root) else {
        return 0;
    };
    let mut painted = 0;
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let Some(node) = ui.node(id) else {
            continue;
        };
        stack.extend(node.children.iter().copied());
        if !node.changed.paint {
            continue;
        }
        painted += 1;
        // A field places its caret and scrolls its text from the box that it was laid out in.
        if ui.node(id).is_some_and(|node| node.field.is_some()) {
            ui.settle_field(id);
        }
        let primitives = primitives(ui, id, settings.scale, settings.direction);
        let node = ui.node_mut(id).expect("walked above");
        node.primitives = primitives;
        node.changed.paint = false;
    }
    painted
}

/// What `id` draws: its shadow, its box, then its text or its image, or a field's selection, text
/// and caret, cut to its content box.
fn primitives(
    ui: &mut Ui,
    id: ElementId,
    scale: f32,
    direction: LayoutDirection,
) -> Vec<(Primitive, Option<Rect>)> {
    let node = ui.node(id).expect("an element");
    let rect = node.rect;
    let content = node.content_box();
    let element = &node.element;
    let paint = node.style.paint;
    let mut boxes = Vec::new();
    box_primitives(&paint, rect, scale, direction, &mut boxes);
    let mut primitives: Vec<(Primitive, Option<Rect>)> = boxes
        .into_iter()
        .map(|primitive| (primitive, None))
        .collect();
    let text_style = node.style.text.style.clone();
    let color = node.style.text.color;
    match &element.kind {
        Kind::Label(text) | Kind::Button { label: text, .. } | Kind::Tab { title: text } => {
            let centred = matches!(element.kind, Kind::Button { .. } | Kind::Tab { .. });
            let text = text.clone();
            let ui = &mut *ui;
            let layout = ui
                .texts
                .get(
                    &mut ui.fonts,
                    &text,
                    &text_style,
                    scale,
                    TextBounds::width(content.width / scale),
                )
                .clone();
            // A button's label in the middle of its box, as browsers place it; a label's at its
            // top, as CSS does.
            let y = if centred {
                content.y + (content.height - layout.height()) / 2.0
            } else {
                content.y
            };
            primitives.push((
                Primitive::Text(TextPrimitive {
                    origin: [content.x, y],
                    layout,
                    color,
                }),
                None,
            ));
        }
        Kind::Image {
            image,
            part,
            kind,
            color,
        } => primitives.push((
            Primitive::Image(ImagePrimitive {
                rect: content,
                image: *image,
                part: *part,
                kind: *kind,
                color: *color,
            }),
            None,
        )),
        Kind::TextField { .. } | Kind::NumberField { .. } => {
            for primitive in ui.field_primitives(id) {
                primitives.push((primitive, Some(content)));
            }
        }
        Kind::CheckBox { .. } | Kind::Slider { .. } | Kind::Split { .. } => {
            for primitive in ui.control_primitives(id) {
                primitives.push((primitive, Some(content)));
            }
        }
        // A section's header lies within its padding, above its content box.
        Kind::Section { .. } => {
            for primitive in ui.control_primitives(id) {
                primitives.push((primitive, None));
            }
        }
        Kind::Container | Kind::Scroll | Kind::List { .. } | Kind::TabBar { .. } => {}
    }
    primitives
}

/// The shadow and the box that `paint` draws for a box at `rect`, its lengths in logical pixels
/// turned into pixels of the screen and its corners mirrored for a root laid out from right to
/// left.
fn box_primitives(
    paint: &PaintStyle,
    rect: Rect,
    scale: f32,
    direction: LayoutDirection,
    primitives: &mut Vec<Primitive>,
) {
    // The canvas's corners: top left, top right, bottom right, bottom left.
    let [top_start, top_end, bottom_end, bottom_start] = paint.radii.map(|radius| radius * scale);
    let radii = match direction {
        LayoutDirection::LeftToRight => [top_start, top_end, bottom_end, bottom_start],
        LayoutDirection::RightToLeft => [top_end, top_start, bottom_start, bottom_end],
    };
    if let Some(shadow) = paint.shadow {
        let along = match direction {
            LayoutDirection::LeftToRight => shadow.offset[0],
            LayoutDirection::RightToLeft => -shadow.offset[0],
        };
        primitives.push(Primitive::Shadow(ShadowPrimitive {
            rect,
            radii,
            color: shadow.color,
            offset: [along * scale, shadow.offset[1] * scale],
            blur: shadow.blur * scale,
            spread: shadow.spread * scale,
        }));
    }
    let border = paint.border_width > 0.0 && paint.border.a > 0.0;
    if paint.background.a > 0.0 || border {
        primitives.push(Primitive::Box(BoxPrimitive {
            rect,
            radii,
            fill: paint.background,
            border_width: if border {
                paint.border_width * scale
            } else {
                0.0
            },
            border: paint.border,
        }));
    }
}

/// Adds to `list` what `root` and the elements below it draw, each before its children, those not
/// laid out left out; a box that scrolls and a list, their scroll bars after their children, over
/// them.
pub(crate) fn gather(ui: &Ui, root: ElementId, list: &mut DisplayList) {
    // An element entered, its own then its children's, or left, what it draws over them; with the
    // opacity of its ancestors and its own.
    let mut stack = vec![(root, true, 1.0_f32)];
    while let Some((id, entered, opacity)) = stack.pop() {
        let Some(node) = ui.node(id) else {
            continue;
        };
        let clip = node.clip.map(snap);
        if !entered {
            for primitive in ui.bar_primitives(id) {
                list.push(primitive.snapped().faded(opacity), clip);
            }
            continue;
        }
        if node.style.layout.display == Display::None {
            continue;
        }
        let opacity = opacity * node.style.paint.opacity.clamp(0.0, 1.0);
        for (primitive, own) in &node.primitives {
            let clip = match own {
                Some(own) => Some(snap(crate::layout::intersect(node.clip, *own))),
                None => clip,
            };
            list.push(primitive.clone().snapped().faded(opacity), clip);
        }
        if matches!(node.element.kind, Kind::Scroll | Kind::List { .. }) {
            stack.push((id, false, opacity));
        }
        stack.extend(
            node.children
                .iter()
                .rev()
                .map(|&child| (child, true, opacity)),
        );
    }
}
