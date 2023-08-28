use std::{
    fmt::{Debug, Display},
    ptr::{null, null_mut},
    sync::Arc,
};

pub use font_kit::family_name::FamilyName as FontFamily;
use harfbuzz::sys::{
    hb_buffer_add, hb_buffer_create, hb_buffer_destroy, hb_buffer_get_glyph_infos,
    hb_buffer_get_glyph_positions, hb_buffer_get_length, hb_buffer_guess_segment_properties,
    hb_buffer_reset, hb_buffer_set_content_type, hb_buffer_t, hb_face_create, hb_face_destroy,
    hb_face_t, hb_font_create, hb_font_destroy, hb_font_get_ppem, hb_font_get_scale, hb_font_t,
    hb_shape, HB_BUFFER_CONTENT_TYPE_UNICODE,
};
use icu_provider_blob::BlobDataProvider;
use icu_segmenter::GraphemeClusterSegmenter;
use itertools::Itertools;
use ropey::RopeSlice;
use swash::FontRef;

#[derive(Debug)]
pub enum FontKitError {
    FontLoadingError(font_kit::error::FontLoadingError),
    SelectionError(font_kit::error::SelectionError),
}

impl Display for FontKitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for FontKitError {}

impl From<font_kit::error::FontLoadingError> for FontKitError {
    fn from(value: font_kit::error::FontLoadingError) -> Self {
        FontKitError::FontLoadingError(value)
    }
}

impl From<font_kit::error::SelectionError> for FontKitError {
    fn from(value: font_kit::error::SelectionError) -> Self {
        FontKitError::SelectionError(value)
    }
}

pub struct Font {
    raw: Arc<Vec<u8>>,
    blob: harfbuzz::Blob<'static>,
    hb_face: *mut hb_face_t,
    hb_font: *mut hb_font_t,
    hb_buffer: *mut hb_buffer_t,
}

impl Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Font{{}}")
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            hb_buffer_destroy(self.hb_buffer);
            hb_font_destroy(self.hb_font);
            hb_face_destroy(self.hb_face);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Glyph {
    pub id: u32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub x_advance: i32,
    pub y_advance: i32,
}

impl Font {
    // pub fn render(&self, glyphs: &[Glyph]) {
    //     //let transform = Transform2F::default();
    //     for glyph in glyphs {
    //         //glyph.id
    //         self.raw.raster_bounds(
    //             glyph_id,
    //             point_size,
    //             transform,
    //             hinting_options,
    //             rasterization_options,
    //         )
    //     }
    //     self.raw.rasterize_glyph(
    //         canvas,
    //         glyph_id,
    //         point_size,
    //         transform,
    //         hinting_options,
    //         rasterization_options,
    //     )
    // }

    //     pub fn shape(&self, text: &str) -> Vec<Glyph> {
    //         unsafe {
    //             hb_buffer_reset(self.hb_buffer);
    //             hb_buffer_set_content_type(self.hb_buffer, HB_BUFFER_CONTENT_TYPE_UNICODE);
    //             //hb_buffer_get_replacement_codepoint(self.hb_buffer)
    //         }
    //         text.chars().for_each(|c| {
    //             let code_point: u32 = c.into();
    //             unsafe { hb_buffer_add(self.hb_buffer, code_point, 0) };
    //         });
    //         unsafe {
    //             hb_buffer_guess_segment_properties(self.hb_buffer);
    //             hb_shape(self.hb_font, self.hb_buffer, null(), 0);
    //             let len = hb_buffer_get_length(self.hb_buffer) as usize;
    //             let info = hb_buffer_get_glyph_infos(self.hb_buffer, null_mut());
    //             let pos = hb_buffer_get_glyph_positions(self.hb_buffer, null_mut());
    //             (0..len)
    //                 .map(|offset| {
    //                     let codepoint = (*info.add(offset)).codepoint;
    //                     let pos = *pos.add(offset);
    //                     Glyph {
    //                         codepoint,
    //                         x_offset: pos.x_offset,
    //                         x_advance: pos.x_advance,
    //                         y_offset: pos.y_offset,
    //                         y_advance: pos.y_advance,
    //                     }
    //                 })
    //                 .collect()
    //         }
    //     }
}

pub struct FontSource {
    raw: font_kit::source::SystemSource,
}

impl FontSource {
    /// Creates a new [`Source`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Finds and loads a font matching the set of provided family priorities.
    pub fn load(&mut self, families: &[FontFamily]) -> Result<Font, FontKitError> {
        let handle = self
            .raw
            .select_best_match(families, &font_kit::properties::Properties::default())?;

        let (data, index) = match handle {
            font_kit::handle::Handle::Path { path, font_index } => {
                use std::io::Read;

                dbg!(&path);
                let mut buf = Vec::new();
                let mut reader =
                    std::fs::File::open(path).map_err(font_kit::error::FontLoadingError::Io)?;
                let _ = reader.read_to_end(&mut buf);

                (Arc::new(buf), font_index)
            }
            font_kit::handle::Handle::Memory { bytes, font_index } => (bytes, font_index),
        };
        let blob = harfbuzz::Blob::new_from_arc_vec(data.clone());
        let (hb_face, hb_font, hb_buffer) = unsafe {
            let face = hb_face_create(blob.as_raw(), index);
            let hb_font = hb_font_create(face);
            let mut x_scale: i32 = 0;
            let mut y_scale: i32 = 0;
            hb_font_get_scale(hb_font, &mut x_scale, &mut y_scale);
            dbg!(x_scale, y_scale);
            // x_scale *= 2;
            // y_scale *= 2;
            // hb_font_set_scale(hb_font, x_scale, y_scale);
            let mut x_ppem: u32 = 0;
            let mut y_ppem: u32 = 0;
            hb_font_get_ppem(hb_font, &mut x_ppem, &mut y_ppem);
            dbg!(x_ppem, y_ppem);
            //hb_font_set_ppem(hb_font, 62, 62);
            let hb_buffer = hb_buffer_create();
            (face, hb_font, hb_buffer)
        };
        Ok(Font {
            raw: data,
            hb_font,
            blob,
            hb_face,
            hb_buffer,
        })
    }

    // pub fn fontref(&self) -> FontRef<'_> {

    // }
}

impl Default for FontSource {
    fn default() -> Self {
        Self {
            raw: font_kit::source::SystemSource::new(),
        }
    }
}

pub struct ShapeContext<'a> {
    font: &'a Font,
    hb_buffer: *mut hb_buffer_t,
    cluster_count: u32,
}

impl<'a> ShapeContext<'a> {
    pub fn new(font: &'a Font) -> ShapeContext<'a> {
        let hb_buffer = unsafe {
            let buf = hb_buffer_create();
            hb_buffer_set_content_type(buf, HB_BUFFER_CONTENT_TYPE_UNICODE);
            buf
        };
        ShapeContext {
            font,
            hb_buffer,
            cluster_count: 0,
        }
    }

    pub fn add_cluster(&mut self, cluster: &str) {
        cluster.chars().for_each(|c| {
            let code_point: u32 = c.into();
            unsafe { hb_buffer_add(self.hb_buffer, code_point, self.cluster_count) };
        });
        self.cluster_count += 1;
    }

    pub fn shape(&mut self) -> Vec<Vec<Glyph>> {
        let mut res = Vec::with_capacity(self.cluster_count as usize);
        for _ in 0..(self.cluster_count) {
            res.push(Vec::with_capacity(1));
        }
        unsafe {
            hb_buffer_guess_segment_properties(self.hb_buffer);
            hb_shape(self.font.hb_font, self.hb_buffer, null(), 0);
            let len = hb_buffer_get_length(self.hb_buffer) as usize;
            let info = hb_buffer_get_glyph_infos(self.hb_buffer, null_mut());
            let pos = hb_buffer_get_glyph_positions(self.hb_buffer, null_mut());
            for offset in 0..len {
                let info = *info.add(offset);
                let codepoint = info.codepoint;
                let cluster = info.cluster;
                if cluster >= self.cluster_count {
                    panic!("more clusters than prepared for");
                }
                let pos = *pos.add(offset);
                let g = Glyph {
                    id: codepoint,
                    x_offset: pos.x_offset,
                    x_advance: pos.x_advance,
                    y_offset: pos.y_offset,
                    y_advance: pos.y_advance,
                };
                res[cluster as usize].push(g);
            }
        }
        res
    }

    pub fn reset(&mut self) {
        unsafe {
            hb_buffer_reset(self.hb_buffer);
            hb_buffer_set_content_type(self.hb_buffer, HB_BUFFER_CONTENT_TYPE_UNICODE);
        }
        self.cluster_count = 0;
    }
}

impl<'a> Drop for ShapeContext<'a> {
    fn drop(&mut self) {
        unsafe { hb_buffer_destroy(self.hb_buffer) };
    }
}

pub struct ParseContext {
    blob_data_provider: BlobDataProvider,
    segmenter: GraphemeClusterSegmenter,
}

impl ParseContext {
    pub fn new() -> ParseContext {
        let blob_data =
            std::fs::read("icu_data.postcard").expect("Failed to read icu_data.postcard");
        let blob_data_provider = BlobDataProvider::try_new_from_blob(blob_data.into_boxed_slice())
            .expect("Failed to initialize Data Provider.");
        let segmenter = GraphemeClusterSegmenter::try_new_with_buffer_provider(&blob_data_provider)
            .expect("FAILED");
        ParseContext {
            blob_data_provider,
            segmenter,
        }
    }

    pub fn segment_str<'a, 'b: 'a>(
        &'a self,
        input: RopeSlice<'_>,
        buf: &'b mut String,
    ) -> impl Iterator<Item = (&'b str, usize, usize)> + 'a {
        buf.clear();
        for c in input.chars() {
            buf.push(c);
        }
        self.segmenter
            .segment_str(buf)
            .tuple_windows()
            .map(|(i, j)| (&buf[i..j], i, j))
    }
}
