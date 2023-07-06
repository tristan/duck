use std::{collections::HashMap, sync::Arc};

pub use font_kit::{error::SelectionError as FontError, family_name::FamilyName as FontFamily};
pub use swash::{CacheKey as FontCacheKey, FontRef};

pub struct FontSource {
    raw: font_kit::source::SystemSource,
    fonts: HashMap<FontCacheKey, (u32, Arc<Vec<u8>>)>,
}

impl FontSource {
    /// Creates a new [`Source`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Finds and loads a font matching the set of provided family priorities.
    pub fn load(&mut self, families: &[FontFamily]) -> Result<FontCacheKey, FontError> {
        let font = self
            .raw
            .select_best_match(families, &font_kit::properties::Properties::default())?;

        let buf = match font {
            font_kit::handle::Handle::Path { path, .. } => {
                use std::io::Read;

                dbg!(&path);
                let mut buf = Vec::new();
                let mut reader =
                    std::fs::File::open(path).expect("font selected but no file was found");
                let _ = reader.read_to_end(&mut buf);

                Arc::new(buf)
            }
            font_kit::handle::Handle::Memory { bytes, .. } => bytes,
        };
        let (fontkey, fontoffset) = {
            let fontref = FontRef::from_index(buf.as_slice(), 0).ok_or(FontError::NotFound)?;
            (fontref.key, fontref.offset)
        };
        self.fonts.insert(fontkey, (fontoffset, buf));
        Ok(fontkey)
    }

    pub fn get_fontref(&self, key: FontCacheKey) -> Option<FontRef<'_>> {
        self.fonts.get(&key).map(|(offset, data)| FontRef {
            data: data.as_ref(),
            offset: *offset,
            key,
        })
    }
}

impl Default for FontSource {
    fn default() -> Self {
        Self {
            raw: font_kit::source::SystemSource::new(),
            fonts: HashMap::new(),
        }
    }
}
