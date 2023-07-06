use guillotiere::{size2, SimpleAtlasAllocator};
use wgpu::{
    BindGroup, Extent3d, Queue, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, TextureView,
};

use super::wgpu_context::WgpuContext;

pub struct Atlas {
    allocator: SimpleAtlasAllocator,
    extent: Extent3d,
    format: TextureFormat,
    texture: Texture,
    view: TextureView,
    pub bind_group: BindGroup,
    block_size: u32,
    buffer: Vec<u8>,
    dirty: bool,
}

impl Atlas {
    pub fn new(wgpu: &WgpuContext, max_size: u32, format: TextureFormat) -> Atlas {
        let block_size = format.block_size(None).expect("Unsupported texture format");
        let buffer = vec![0u8; (max_size * max_size * block_size) as usize];
        let extent = Extent3d {
            width: max_size,
            height: max_size,
            depth_or_array_layers: 1,
        };
        let texture = wgpu.device.create_texture(&TextureDescriptor {
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            label: Some("altas_texture"),
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &wgpu.texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&wgpu.texture_sampler),
                },
            ],
            label: Some("altas_texture_bind_group"),
        });
        Atlas {
            allocator: SimpleAtlasAllocator::new(size2(max_size as i32, max_size as i32)),
            extent,
            format,
            texture,
            view,
            bind_group,
            block_size,
            buffer,
            dirty: true,
        }
    }

    pub fn update_texture(&mut self, queue: &Queue) {
        if !self.dirty || self.allocator.is_empty() {
            return;
        }
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &self.buffer,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.block_size * self.extent.width),
                rows_per_image: Some(self.extent.height),
            },
            self.extent,
        );
        self.dirty = false;
    }

    pub fn allocate(&mut self, width: u32, height: u32, data: &[u8]) -> Option<(u32, u32)> {
        let (x, y) = self
            .allocator
            .allocate(size2(width as i32, height as i32))
            .map(|rect| (rect.min.x as u32, rect.min.y as u32))?;
        let channels = self.block_size as usize;
        let data_stride = width as usize * channels;
        let buffer_stride = self.extent.width as usize * channels;
        let mut offset = y as usize * buffer_stride + x as usize * channels;
        for row in data.chunks(data_stride) {
            let dest = self.buffer.get_mut(offset..offset + data_stride)?;
            dest.copy_from_slice(row);
            offset += buffer_stride;
        }
        self.dirty = true;
        Some((x, y))
    }
}
