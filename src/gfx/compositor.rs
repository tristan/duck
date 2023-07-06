use super::{
    color::Color,
    image_cache::TextureLocation,
    types::{Rect, Vertex},
};

#[derive(Default)]
struct Batch {
    atlas_index: Option<usize>,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Batch {
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.atlas_index = None;
    }

    fn add_rect(
        &mut self,
        rect: Rect,
        depth: f32,
        color: Color,
        coords: Option<&[f32; 4]>,
        atlas_index: Option<usize>,
    ) {
        self.atlas_index = atlas_index;
        let x = rect.x;
        let y = rect.y;
        let w = rect.width;
        let h = rect.height;
        let l = coords.map(|c| c[0]).unwrap_or(0.);
        let t = coords.map(|c| c[1]).unwrap_or(0.);
        let r = coords.map(|c| c[2]).unwrap_or(1.);
        let b = coords.map(|c| c[3]).unwrap_or(1.);
        let flags = if coords.is_some() { 1. } else { 0. };
        let verts = [
            Vertex {
                pos: [x, y, depth, flags],
                color,
                uv: [l, t],
            },
            Vertex {
                pos: [x, y + h, depth, flags],
                color,
                uv: [l, b],
            },
            Vertex {
                pos: [x + w, y + h, depth, flags],
                color,
                uv: [r, b],
            },
            Vertex {
                pos: [x + w, y, depth, flags],
                color,
                uv: [r, t],
            },
        ];
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&verts);
        self.indices.extend_from_slice(&[
            base, //
            base + 1,
            base + 2,
            base,
            base + 2,
            base + 3,
        ]);
    }

    fn build_display_list(&self, list: &mut DisplayList) {
        let first_vertex = list.vertices.len() as u32;
        let first_index = list.indices.len() as u32;
        list.vertices.extend_from_slice(&self.vertices);
        list.indices
            .extend(self.indices.iter().map(|i| *i + first_vertex));
        if let Some(atlas_index) = self.atlas_index {
            list.commands.push(Command::BindTexture(atlas_index));
        }
        list.commands.push(Command::Draw {
            start: first_index,
            count: self.indices.len() as u32,
        });
    }
}

#[derive(Clone, Copy)]
enum BatchType {
    Opaque,
    Transparent,
    Subpixel,
}

pub struct Compositor {
    empty_batches: Vec<Batch>,
    opaque_batches: Vec<Batch>,
    transparent_batches: Vec<Batch>,
    subpixel_batches: Vec<Batch>,
}

impl Compositor {
    pub fn new() -> Compositor {
        Compositor {
            empty_batches: Vec::new(),
            opaque_batches: Vec::new(),
            transparent_batches: Vec::new(),
            subpixel_batches: Vec::new(),
        }
    }

    pub fn begin(&mut self) {
        self.empty_batches.append(&mut self.opaque_batches);
        self.empty_batches.append(&mut self.transparent_batches);
        self.empty_batches
            .iter_mut()
            .for_each(|batch| batch.clear());
    }

    pub fn draw_rect(&mut self, rect: impl Into<Rect>, depth: f32, color: Color) {
        let batch_type = if color.a == 255 {
            BatchType::Opaque
        } else {
            BatchType::Transparent
        };
        let batch = match self.get_batch(batch_type, None) {
            Some(batch) => batch,
            None => self.allocate_batch(batch_type),
        };
        batch.add_rect(rect.into(), depth, color, None, None);
    }

    pub fn add_image_rect(
        &mut self,
        rect: impl Into<Rect>,
        depth: f32,
        color: Color,
        texture_location: TextureLocation,
    ) {
        let atlas_index = Some(texture_location.atlas_index);
        let batch = match self.get_batch(BatchType::Transparent, atlas_index) {
            Some(batch) => batch,
            None => self.allocate_batch(BatchType::Transparent),
        };
        let coords = [
            texture_location.min.0,
            texture_location.min.1,
            texture_location.max.0,
            texture_location.max.1,
        ];
        batch.add_rect(rect.into(), depth, color, Some(&coords), atlas_index);
    }

    pub fn add_subpixel_rect(
        &mut self,
        rect: impl Into<Rect>,
        depth: f32,
        color: Color,
        texture_location: TextureLocation,
    ) {
        let atlas_index = Some(texture_location.atlas_index);
        let batch = match self.get_batch(BatchType::Subpixel, atlas_index) {
            Some(batch) => batch,
            None => self.allocate_batch(BatchType::Subpixel),
        };
        let coords = [
            texture_location.min.0,
            texture_location.min.1,
            texture_location.max.0,
            texture_location.max.1,
        ];
        batch.add_rect(rect.into(), depth, color, Some(&coords), atlas_index);
    }

    fn get_batch(
        &mut self,
        batch_type: BatchType,
        atlas_index: Option<usize>,
    ) -> Option<&mut Batch> {
        let check_fn = |batch: &&mut Batch| -> bool {
            if atlas_index.is_some() && batch.atlas_index.is_some() {
                atlas_index == batch.atlas_index
            } else {
                true
            }
        };
        match batch_type {
            BatchType::Transparent => self.transparent_batches.iter_mut().find(check_fn),
            BatchType::Subpixel => self.subpixel_batches.iter_mut().find(check_fn),
            BatchType::Opaque => self.opaque_batches.iter_mut().find(check_fn),
        }
    }

    fn allocate_batch(&mut self, batch_type: BatchType) -> &mut Batch {
        let batch = if let Some(batch) = self.empty_batches.pop() {
            batch
        } else {
            Batch::default()
        };
        match batch_type {
            BatchType::Transparent => {
                self.transparent_batches.push(batch);
                self.transparent_batches.last_mut().unwrap()
            }
            BatchType::Subpixel => {
                self.subpixel_batches.push(batch);
                self.subpixel_batches.last_mut().unwrap()
            }
            BatchType::Opaque => {
                self.opaque_batches.push(batch);
                self.opaque_batches.last_mut().unwrap()
            }
        }
    }

    pub fn build_display_list(&self) -> DisplayList {
        let mut list = DisplayList::new();
        if !self.opaque_batches.is_empty() {
            list.commands.push(Command::BindPipeline(Pipeline::Opaque));
            for batch in &self.opaque_batches {
                if batch.vertices.is_empty() {
                    continue;
                }
                batch.build_display_list(&mut list);
            }
        }
        if !self.transparent_batches.is_empty() {
            list.commands
                .push(Command::BindPipeline(Pipeline::Transparent));
            for batch in &self.transparent_batches {
                if batch.vertices.is_empty() {
                    continue;
                }
                batch.build_display_list(&mut list);
            }
        }
        if !self.subpixel_batches.is_empty() {
            list.commands
                .push(Command::BindPipeline(Pipeline::Subpixel));
            for batch in &self.subpixel_batches {
                if batch.vertices.is_empty() {
                    continue;
                }
                batch.build_display_list(&mut list);
            }
        }
        list
    }
}

/// Resources and commands for drawing a composition.
#[derive(Default, Clone)]
pub struct DisplayList {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    commands: Vec<Command>,
}

impl DisplayList {
    /// Creates a new empty display list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the buffered vertices for the display list.
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    /// Returns the buffered indices for the display list.
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Returns the sequence of display commands.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// Clears the display list.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
    }
}

/// Command in a display list.
#[derive(Copy, Clone, Debug)]
pub enum Command {
    /// Bind a texture at the specified slot.
    BindTexture(usize),
    /// Switch to the specified render mode.
    BindPipeline(Pipeline),
    /// Draw the specified range of indexed triangles.
    Draw { start: u32, count: u32 },
}

/// Pipelines used by a display list.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Pipeline {
    Opaque,
    Transparent,
    Subpixel,
}
