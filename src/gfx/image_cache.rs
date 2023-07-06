use wgpu::{BindGroup, TextureFormat};

use super::{atlas::Atlas, wgpu_context::WgpuContext};

pub struct ImageCache {
    atlases: Vec<Atlas>,
    entries: Vec<TextureLocation>,
    max_texture_size: u32,
}

impl ImageCache {
    pub fn new(max_texture_size: u32) -> ImageCache {
        ImageCache {
            atlases: Vec::new(),
            entries: Vec::new(),
            max_texture_size,
        }
    }

    pub fn atlas_iter_mut(&mut self) -> impl Iterator<Item = &mut Atlas> {
        self.atlases.iter_mut()
    }

    pub fn get_bind_group(&self, index: usize) -> Option<&BindGroup> {
        Some(&self.atlases.get(index)?.bind_group)
    }

    pub fn allocate(
        &mut self,
        wgpu: &WgpuContext,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Option<usize> {
        let entry = 'outer: {
            for (atlas_index, atlas) in self.atlases.iter_mut().enumerate() {
                if let Some((x, y)) = atlas.allocate(width, height, data) {
                    break 'outer Some(Entry {
                        atlas_index,
                        x,
                        y,
                        width,
                        height,
                    });
                }
            }
            let atlas_index = self.atlases.len();
            let atlas = Atlas::new(wgpu, self.max_texture_size, TextureFormat::Rgba8UnormSrgb);
            self.atlases.push(atlas);
            let atlas = self.atlases.last_mut().unwrap();
            if let Some((x, y)) = atlas.allocate(width, height, data) {
                Some(Entry {
                    atlas_index,
                    x,
                    y,
                    width,
                    height,
                })
            } else {
                log::error!("Unable to allocate atlas for size: {}x{}", width, height);
                None
            }
        }?;
        let id = self.entries.len();
        let s = 1. / self.max_texture_size as f32;
        let location = TextureLocation {
            atlas_index: entry.atlas_index,
            min: (entry.x as f32 * s, entry.y as f32 * s),
            max: (
                (entry.x + entry.width) as f32 * s,
                (entry.y + entry.height) as f32 * s,
            ),
        };
        self.entries.push(location);
        Some(id)
    }

    pub fn get_image_location(&self, image_id: usize) -> Option<TextureLocation> {
        self.entries.get(image_id).copied()
    }
}

struct Entry {
    atlas_index: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TextureLocation {
    pub atlas_index: usize,
    pub min: (f32, f32),
    pub max: (f32, f32),
}
