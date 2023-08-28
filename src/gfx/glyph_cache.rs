#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;

use swash::{
    scale::{
        image::{Content, Image as GlyphImage},
        Render, ScaleContext, Scaler, Source, StrikeWith,
    },
    zeno::{Format, Vector},
    CacheKey as FontCacheKey, FontRef, GlyphId,
};

use super::{
    image_cache::{ImageCache, TextureLocation},
    wgpu_context::WgpuContext,
};

const IS_MACOS: bool = cfg!(target_os = "macos");

const SOURCES: &[Source] = &[
    Source::ColorBitmap(StrikeWith::BestFit),
    Source::ColorOutline(0),
    //Source::Bitmap(Strike::ExactSize),
    Source::Outline,
];

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
struct GlyphKey {
    fontkey: FontCacheKey,
    id: GlyphId,
    offset: [SubpixelOffset; 2],
    size: u16,
}

#[derive(Default)]
pub struct GlyphCache {
    scale_context: ScaleContext,
    img: GlyphImage,
    glyphs: HashMap<GlyphKey, GlyphEntry>,
}

impl GlyphCache {
    pub fn new() -> GlyphCache {
        GlyphCache::default()
    }

    pub fn session<'a>(
        &'a mut self,
        wgpu: &'a WgpuContext,
        image_cache: &'a mut ImageCache,
        fontref: &Font,
        size: f32,
        coords: &[i16],
    ) -> GlyphCacheSession<'a> {
        let quant_size = (size * 32.) as u16;
        let fontkey = fontref.key;
        let scaler = self
            .scale_context
            .builder(fontref)
            .hint(!IS_MACOS)
            .size(size)
            .normalized_coords(coords)
            .build();
        GlyphCacheSession {
            wgpu,
            image_cache,
            scaler,
            img: &mut self.img,
            quant_size,
            glyphs: &mut self.glyphs,
            fontkey,
        }
    }
}

pub struct GlyphCacheSession<'a> {
    wgpu: &'a WgpuContext,
    image_cache: &'a mut ImageCache,
    scaler: Scaler<'a>,
    img: &'a mut GlyphImage,
    quant_size: u16,
    glyphs: &'a mut HashMap<GlyphKey, GlyphEntry>,
    fontkey: FontCacheKey,
}

impl<'a> GlyphCacheSession<'a> {
    pub fn get_texture_location(&self, image_id: usize) -> Option<TextureLocation> {
        // this is added as a wrapper function as the session is holding onto a
        // mutable reference to the image cache
        self.image_cache.get_image_location(image_id)
    }

    pub fn get(&mut self, id: GlyphId, x: f32, y: f32) -> Option<GlyphEntry> {
        let subpx = [SubpixelOffset::quantize(x), SubpixelOffset::quantize(y)];
        let key = GlyphKey {
            id,
            fontkey: self.fontkey,
            offset: subpx,
            size: self.quant_size,
        };
        if let Some(entry) = self.glyphs.get(&key) {
            return Some(*entry);
        }
        self.img.clear();
        let embolden = if IS_MACOS { 0.25 } else { 0. };
        if Render::new(SOURCES)
            .format(Format::CustomSubpixel([0.3, 0., -0.3]))
            .offset(Vector::new(subpx[0].to_f32(), subpx[1].to_f32()))
            .embolden(embolden)
            .render_into(&mut self.scaler, id, self.img)
        {
            let p = self.img.placement;
            let left = p.left;
            let top = p.top;
            let width = p.width;
            let height = p.height;
            if width == 0 || height == 0 {
                return None;
            }
            let is_bitmap = self.img.content == Content::Color;
            //dbg!(self.img.content);
            // let mut rgba8 = image::RgbaImage::new(width, height);
            // for y in 0..height {
            //     for x in 0..width {
            //         let slice = &self.img.data[(y * width * 4 + (x * 4)) as usize..];
            //         let pixel = image::Rgba([slice[0], slice[1], slice[2], 255]);
            //         rgba8.put_pixel(x, y, pixel);
            //     }
            // }
            //rgba8.save(format!("{id}_{x}_{y}.bmp")).unwrap();
            let image_id = self
                .image_cache
                .allocate(self.wgpu, width, height, &self.img.data)?;
            let entry = GlyphEntry {
                left,
                top,
                width,
                height,
                is_bitmap,
                image_id,
            };
            self.glyphs.insert(key, entry);
            Some(entry)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphEntry {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub is_bitmap: bool,
    pub image_id: usize,
}

#[derive(Hash, Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SubpixelOffset {
    Zero = 0,
    Quarter = 1,
    Half = 2,
    ThreeQuarters = 3,
}

impl SubpixelOffset {
    // Skia quantizes subpixel offsets into 1/4 increments.
    // Given the absolute position, return the quantized increment
    pub fn quantize(pos: f32) -> Self {
        // Following the conventions of Gecko and Skia, we want
        // to quantize the subpixel position, such that abs(pos) gives:
        // [0.0, 0.125) -> Zero
        // [0.125, 0.375) -> Quarter
        // [0.375, 0.625) -> Half
        // [0.625, 0.875) -> ThreeQuarters,
        // [0.875, 1.0) -> Zero
        // The unit tests below check for this.
        let apos = ((pos - pos.floor()) * 8.0) as i32;
        match apos {
            1..=2 => SubpixelOffset::Quarter,
            3..=4 => SubpixelOffset::Half,
            5..=6 => SubpixelOffset::ThreeQuarters,
            _ => SubpixelOffset::Zero,
        }
    }

    pub fn to_f32(self) -> f32 {
        match self {
            SubpixelOffset::Zero => 0.0,
            SubpixelOffset::Quarter => 0.25,
            SubpixelOffset::Half => 0.5,
            SubpixelOffset::ThreeQuarters => 0.75,
        }
    }
}
