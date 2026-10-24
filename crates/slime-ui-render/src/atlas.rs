//! The glyphs' atlas: textures that hold each glyph's image once, for every text that draws it.

use std::collections::{HashMap, HashSet};

use etagere::{AllocId, AtlasAllocator, size2};
use slime_gfx::{GfxError, Gpu, Texture, TextureFormat};
use slime_ui::text::{Fonts, GlyphKey, GlyphPixels, GlyphRasterizer};

/// The side of an atlas page, in texels: a page of coverage takes 1 MiB, one of colors 4 MiB. A
/// glyph's image larger than a page, less the empty texel around it, is never drawn.
pub const ATLAS_PAGE_SIZE: u32 = 1024;

/// The memory that a [`UiRenderer`](crate::UiRenderer)'s atlas pages take at most by default: 16
/// MiB, sixteen pages of coverage, some 50 000 glyphs of a text of 14 pixels.
pub const GLYPH_ATLAS_BUDGET: u64 = 16 << 20;

/// How many draws a glyph that draws nothing stays known without one asking for it.
const KEPT_WITHOUT_IMAGE: u64 = 256;

/// Where a glyph's image lies in the atlas, and how it is placed against its origin.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Place {
    pub(crate) page: usize,
    /// The image's top left texel in its page, inside the empty texel that borders it.
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// From the glyph's origin to the image's left edge, right positive, and to its top edge, up
    /// positive, in pixels.
    pub(crate) left: i32,
    pub(crate) top: i32,
    /// Whether the image is of colors, an emoji's, rather than of coverage.
    pub(crate) color: bool,
}

/// A page: a texture, and the allocator that places images in it.
struct Page {
    texture: Texture,
    allocator: AtlasAllocator,
    color: bool,
}

/// What the atlas knows of a glyph.
#[derive(Clone, Copy)]
enum Slot {
    /// Its image, in a page.
    Image(Place, AllocId),
    /// It draws nothing: a space, or a glyph that its face cannot draw.
    Nothing,
    /// Its image is larger than a page.
    Oversized,
}

/// A glyph that a draw asked for, and the draw that asked for it last.
struct Entry {
    slot: Slot,
    used: u64,
    /// The order in which glyphs came, which orders those that a draw last asked for together.
    order: u64,
}

/// What a draw did to the atlas.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Placed {
    pub(crate) rasterized: usize,
    pub(crate) left_out: usize,
}

/// The pages, and the glyphs they hold, within a budget of memory.
pub(crate) struct Atlas {
    /// The pages by their numbers; none where a page was let go, a number that the next page
    /// takes.
    pages: Vec<Option<Page>>,
    glyphs: HashMap<GlyphKey, Entry>,
    rasterizer: GlyphRasterizer,
    budget: u64,
    bytes: u64,
    /// The draw under way, which numbers each draw.
    frame: u64,
    /// The glyphs that came, which numbers each.
    count: u64,
    /// Of coverage and of colors, whether the draw under way found no more room for a new glyph.
    full: [bool; 2],
    /// The glyphs that the draw under way left out.
    left_out: HashSet<GlyphKey>,
    warned: bool,
}

impl Atlas {
    pub(crate) fn new(budget: u64) -> Self {
        Self {
            pages: Vec::new(),
            glyphs: HashMap::new(),
            rasterizer: GlyphRasterizer::new(),
            budget,
            bytes: 0,
            frame: 0,
            count: 0,
            full: [false; 2],
            left_out: HashSet::new(),
            warned: false,
        }
    }

    /// Starts a draw: the glyphs that it asks for stay in the atlas until it ends. Now and then,
    /// it forgets the glyphs that draw nothing and that no draw asked for in a while.
    pub(crate) fn begin(&mut self) {
        self.frame += 1;
        self.full = [false; 2];
        self.left_out.clear();
        if self.frame.is_multiple_of(KEPT_WITHOUT_IMAGE) {
            let frame = self.frame;
            self.glyphs.retain(|_, entry| {
                matches!(entry.slot, Slot::Image(..)) || entry.used + KEPT_WITHOUT_IMAGE > frame
            });
        }
    }

    /// The memory that the pages take.
    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }

    /// How many page numbers there are, some of them let go.
    pub(crate) fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// The texture of each page there is, with its number.
    pub(crate) fn textures(&self) -> impl Iterator<Item = (usize, &Texture)> {
        self.pages
            .iter()
            .enumerate()
            .filter_map(|(index, page)| Some((index, &page.as_ref()?.texture)))
    }

    /// Where the image of the glyph `key` lies: kept from an earlier draw, or rasterized with
    /// `fonts` and written into a page now. None for a glyph that draws nothing, or that no page
    /// has room for within the budget, the glyphs of this draw kept, or larger than a page.
    ///
    /// # Errors
    ///
    /// [`GfxError::OutOfMemory`] or [`GfxError::DeviceLost`] if a page cannot be made.
    pub(crate) fn place(
        &mut self,
        gpu: &Gpu,
        fonts: &Fonts,
        key: GlyphKey,
        placed: &mut Placed,
    ) -> Result<Option<Place>, GfxError> {
        let frame = self.frame;
        if let Some(entry) = self.glyphs.get_mut(&key) {
            entry.used = frame;
            return Ok(match entry.slot {
                Slot::Image(place, _) => Some(place),
                Slot::Nothing => None,
                Slot::Oversized => {
                    placed.left_out += 1;
                    None
                }
            });
        }
        if self.left_out.contains(&key) {
            placed.left_out += 1;
            return Ok(None);
        }
        placed.rasterized += 1;
        let image = self
            .rasterizer
            .rasterize(fonts, key)
            .filter(|image| image.width > 0 && image.height > 0);
        let Some(image) = image else {
            self.remember(key, Slot::Nothing);
            return Ok(None);
        };
        let (pixels, color, texel) = match &image.pixels {
            GlyphPixels::Coverage(pixels) => (pixels, false, 1),
            GlyphPixels::Color(pixels) => (pixels, true, 4),
        };
        // An empty texel around each image, so that filtering at its edge reads nothing of another.
        let (width, height) = (image.width + 2, image.height + 2);
        if width > ATLAS_PAGE_SIZE || height > ATLAS_PAGE_SIZE {
            self.remember(key, Slot::Oversized);
            self.leave_out(key, placed);
            return Ok(None);
        }
        let found = if self.full[usize::from(color)] {
            None
        } else {
            self.allocate(gpu, color, width, height)?
        };
        let Some((page, id, x, y)) = found else {
            self.full[usize::from(color)] = true;
            self.leave_out(key, placed);
            return Ok(None);
        };
        let row = (width * texel) as usize;
        let mut padded = vec![0; row * height as usize];
        let glyph_row = (image.width * texel) as usize;
        for (index, source) in pixels.chunks_exact(glyph_row).enumerate() {
            let start = (index + 1) * row + texel as usize;
            padded[start..start + glyph_row].copy_from_slice(source);
        }
        let texture = &self.pages[page].as_ref().expect("allocated in it").texture;
        gpu.write_texture(texture, (x, y), (width, height), &padded);
        let place = Place {
            page,
            x: x + 1,
            y: y + 1,
            width: image.width,
            height: image.height,
            left: image.left,
            top: image.top,
            color,
        };
        self.remember(key, Slot::Image(place, id));
        Ok(Some(place))
    }

    fn remember(&mut self, key: GlyphKey, slot: Slot) {
        self.count += 1;
        self.glyphs.insert(
            key,
            Entry {
                slot,
                used: self.frame,
                order: self.count,
            },
        );
    }

    /// Counts `key` left out of this draw, and says so the first time.
    fn leave_out(&mut self, key: GlyphKey, placed: &mut Placed) {
        placed.left_out += 1;
        self.left_out.insert(key);
        if !self.warned {
            tracing::warn!(
                budget = self.budget,
                "glyphs that a draw asks for do not fit the atlas: they are left out (said once \
                 per renderer)"
            );
            self.warned = true;
        }
    }

    /// Room for an image of `width` × `height` texels in a page of colors or of coverage: in a
    /// page that has it; in a new page within the budget; freed by the glyphs of this kind that
    /// no draw asked for the longest; or in a new page, where pages whose glyphs this draw does
    /// not draw are let go, those that no draw asked for the longest first. Returns the page, the
    /// allocation and its top left texel.
    fn allocate(
        &mut self,
        gpu: &Gpu,
        color: bool,
        width: u32,
        height: u32,
    ) -> Result<Option<(usize, AllocId, u32, u32)>, GfxError> {
        let size = size2(width as i32, height as i32);
        let found = |page: usize, allocation: etagere::Allocation| {
            let min = allocation.rectangle.min;
            Some((page, allocation.id, min.x as u32, min.y as u32))
        };
        for (index, page) in self.pages.iter_mut().enumerate() {
            if let Some(page) = page
                && page.color == color
                && let Some(allocation) = page.allocator.allocate(size)
            {
                return Ok(found(index, allocation));
            }
        }
        let page_bytes = page_bytes(color);
        if self.bytes + page_bytes <= self.budget {
            let index = self.make_page(gpu, color)?;
            let allocation = self.pages[index]
                .as_mut()
                .expect("made above")
                .allocator
                .allocate(size)
                .expect("an empty page holds an image no larger than a page");
            return Ok(found(index, allocation));
        }
        // The glyphs of this kind that no draw asked for the longest leave first, until one page
        // has room.
        let mut unused: Vec<(u64, u64, GlyphKey)> = self
            .glyphs
            .iter()
            .filter_map(|(&key, entry)| match entry.slot {
                Slot::Image(place, _) if place.color == color && entry.used < self.frame => {
                    Some((entry.used, entry.order, key))
                }
                _ => None,
            })
            .collect();
        unused.sort_unstable_by_key(|&(used, order, _)| (used, order));
        for (_, _, key) in unused {
            let Some(Entry {
                slot: Slot::Image(place, id),
                ..
            }) = self.glyphs.remove(&key)
            else {
                unreachable!("listed above")
            };
            let page = self.pages[place.page].as_mut().expect("its glyph is in it");
            page.allocator.deallocate(id);
            if let Some(allocation) = page.allocator.allocate(size) {
                return Ok(found(place.page, allocation));
            }
        }
        // Pages, of either kind, whose glyphs this draw does not draw, let go until a page of this
        // kind fits the budget.
        let mut last_used: Vec<Option<u64>> = vec![Some(0); self.pages.len()];
        for (index, page) in self.pages.iter().enumerate() {
            if page.is_none() {
                last_used[index] = None;
            }
        }
        for entry in self.glyphs.values() {
            if let Slot::Image(place, _) = entry.slot
                && let Some(used) = &mut last_used[place.page]
            {
                *used = (*used).max(entry.used);
            }
        }
        let mut idle: Vec<(u64, usize)> = last_used
            .iter()
            .enumerate()
            .filter_map(|(index, used)| {
                used.filter(|&used| used < self.frame)
                    .map(|used| (used, index))
            })
            .collect();
        idle.sort_unstable();
        let freeable: u64 = idle
            .iter()
            .map(|&(_, index)| page_bytes_of(&self.pages[index]))
            .sum();
        if self.bytes - freeable + page_bytes > self.budget {
            return Ok(None);
        }
        for (_, index) in idle {
            if self.bytes + page_bytes <= self.budget {
                break;
            }
            self.let_go(index);
        }
        let index = self.make_page(gpu, color)?;
        let allocation = self.pages[index]
            .as_mut()
            .expect("made above")
            .allocator
            .allocate(size)
            .expect("an empty page holds an image no larger than a page");
        Ok(found(index, allocation))
    }

    /// Makes a page of colors or of coverage, at the first number free, and gives its number.
    fn make_page(&mut self, gpu: &Gpu, color: bool) -> Result<usize, GfxError> {
        let (format, label) = if color {
            (TextureFormat::Rgba8UnormSrgb, "glyphs' atlas, colors")
        } else {
            (TextureFormat::R8Unorm, "glyphs' atlas, coverage")
        };
        let bytes = page_bytes(color);
        let empty = vec![0; bytes as usize];
        let texture = gpu.create_texture_with_levels(
            label,
            ATLAS_PAGE_SIZE,
            ATLAS_PAGE_SIZE,
            format,
            &[&empty],
        )?;
        let side = ATLAS_PAGE_SIZE as i32;
        let page = Page {
            texture,
            allocator: AtlasAllocator::new(size2(side, side)),
            color,
        };
        self.bytes += bytes;
        Ok(match self.pages.iter().position(Option::is_none) {
            Some(index) => {
                self.pages[index] = Some(page);
                index
            }
            None => {
                self.pages.push(Some(page));
                self.pages.len() - 1
            }
        })
    }

    /// Lets go of the page `index` and of the glyphs it holds.
    fn let_go(&mut self, index: usize) {
        let page = self.pages[index].take().expect("a page there");
        self.bytes -= page_bytes(page.color);
        self.glyphs.retain(|_, entry| match entry.slot {
            Slot::Image(place, _) => place.page != index,
            _ => true,
        });
    }
}

/// The memory that a page of colors or of coverage takes.
fn page_bytes(color: bool) -> u64 {
    let texel = if color { 4 } else { 1 };
    u64::from(ATLAS_PAGE_SIZE * ATLAS_PAGE_SIZE * texel)
}

/// The memory that a page takes; none where it was let go.
fn page_bytes_of(page: &Option<Page>) -> u64 {
    page.as_ref().map_or(0, |page| page_bytes(page.color))
}
