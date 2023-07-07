use wgpu::util::DeviceExt;
use winit::window::Window;

use super::{
    compositor::{Command, DisplayList, Pipeline},
    image_cache::ImageCache,
    types::{CameraUniform, Vertex},
};

pub struct WgpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub shader: wgpu::ShaderModule,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_sampler: wgpu::Sampler,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub render_pipeline_layout: wgpu::PipelineLayout,
    pub opaque_render_pipeline: wgpu::RenderPipeline,
    pub transparent_render_pipeline: wgpu::RenderPipeline,
    pub subpixel_r_render_pipeline: wgpu::RenderPipeline,
    pub subpixel_g_render_pipeline: wgpu::RenderPipeline,
    pub subpixel_b_render_pipeline: wgpu::RenderPipeline,

    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,

    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
}

impl WgpuContext {
    pub fn new(window: &Window) -> WgpuContext {
        let scale_factor = window.scale_factor() as f32;
        let size = dbg!(window.inner_size());
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });
        let surface =
            unsafe { instance.create_surface(&window) }.expect("failed to create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("failed to fetch adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .expect("failed to fetch device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        dbg!(surface_format);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        // pipeline setup!
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let (depth_texture, depth_view) = create_depth_texture(&device, size.width, size.height);

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&camera_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let primitive_state = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        };
        let depth_stencil_state = wgpu::DepthStencilState {
            format: DEPTH_TEXTURE_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        };
        let multisample_state = wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        };
        let opaque_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Opaque Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "transparent_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::REPLACE), // TODO: is this right?
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: primitive_state,
                depth_stencil: Some(depth_stencil_state.clone()),
                multisample: multisample_state,
                multiview: None,
            });
        let transparent_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Trnasparent Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "transparent_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: primitive_state,
                depth_stencil: Some(depth_stencil_state.clone()),
                multisample: multisample_state,
                multiview: None,
            });
        let subpixel_r_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Subpixel R Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "subpixel_r_vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "subpixel_r_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::RED | wgpu::ColorWrites::ALPHA,
                    })],
                }),
                primitive: primitive_state,
                depth_stencil: Some(depth_stencil_state.clone()),
                multisample: multisample_state,
                multiview: None,
            });
        let subpixel_g_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Subpixel G Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "subpixel_g_vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "subpixel_g_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::GREEN | wgpu::ColorWrites::ALPHA,
                    })],
                }),
                primitive: primitive_state,
                depth_stencil: Some(depth_stencil_state.clone()),
                multisample: multisample_state,
                multiview: None,
            });
        let subpixel_b_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Subpixel B Render Pipeline"),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "subpixel_b_vs_main",
                    buffers: &[Vertex::desc()],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "subpixel_b_fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: config.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::BLUE | wgpu::ColorWrites::ALPHA,
                    })],
                }),
                primitive: primitive_state,
                depth_stencil: Some(depth_stencil_state),
                multisample: multisample_state,
                multiview: None,
            });

        // data setup!

        let camera_uniform = CameraUniform::new(size.width, size.height, scale_factor);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        WgpuContext {
            instance,
            adapter,
            surface,
            device,
            queue,
            shader,
            config,
            texture_bind_group_layout,
            texture_sampler,
            camera_bind_group_layout,
            depth_texture,
            depth_view,
            render_pipeline_layout,
            opaque_render_pipeline,
            transparent_render_pipeline,
            subpixel_r_render_pipeline,
            subpixel_g_render_pipeline,
            subpixel_b_render_pipeline,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            vertex_buffer: None,
            index_buffer: None,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, scale_factor: f32) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.camera_uniform.update(width, height, scale_factor);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        (self.depth_texture, self.depth_view) = create_depth_texture(&self.device, width, height);
    }

    pub fn render(
        &mut self,
        image_cache: &mut ImageCache,
        display_list: &DisplayList,
    ) -> Result<(), ()> {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(e) => {
                log::error!("Failed to get current texture: {e}");
                match e {
                    wgpu::SurfaceError::Lost => {
                        self.surface.configure(&self.device, &self.config);
                    }
                    wgpu::SurfaceError::OutOfMemory => {
                        return Err(());
                    }
                    _ => {}
                }
                return Ok(());
            }
        };

        // update texture buffers for atlases
        for atlas in image_cache.atlas_iter_mut() {
            atlas.update_texture(&self.queue);
        }

        // TODO: don't recreate each time unless necessary!
        let vertex_data = display_list.vertices();
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            });
        self.vertex_buffer = Some(vertex_buffer);
        let index_data = display_list.indices();
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(index_data),
                usage: wgpu::BufferUsages::INDEX,
            });
        self.index_buffer = Some(index_buffer);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            // bind the camera bind group
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            // we need to bind something to the texture bind group even if we don't use it
            let Some(atlas_bind_group) = image_cache.get_bind_group(0) else {
                log::error!("Missing atlas at index 0");
                return Ok(());
            };
            render_pass.set_bind_group(1, atlas_bind_group, &[]);
            if let (Some(vertex_buffer), Some(index_buffer)) =
                (&self.vertex_buffer, &self.index_buffer)
            {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            }
            //println!("....... >");
            let mut current_pipeline = Pipeline::Opaque;
            for command in display_list.commands() {
                //println!("{:?}", command);
                match *command {
                    Command::BindTexture(atlas_index) => {
                        let Some(atlas_bind_group) = image_cache.get_bind_group(atlas_index) else {
                            log::error!("Missing atlas at index {}", atlas_index);
                            return Ok(());
                        };
                        render_pass.set_bind_group(1, atlas_bind_group, &[]);
                    }
                    Command::Draw { start, count } => {
                        if matches!(current_pipeline, Pipeline::Subpixel) {
                            render_pass.set_pipeline(&self.subpixel_r_render_pipeline);
                            render_pass.draw_indexed(start..(start + count), 0, 0..1);
                            render_pass.set_pipeline(&self.subpixel_g_render_pipeline);
                            render_pass.draw_indexed(start..(start + count), 0, 0..1);
                            render_pass.set_pipeline(&self.subpixel_b_render_pipeline);
                            render_pass.draw_indexed(start..(start + count), 0, 0..1);
                        } else {
                            render_pass.draw_indexed(start..(start + count), 0, 0..1);
                        }
                    }
                    Command::BindPipeline(pipeline) => match pipeline {
                        Pipeline::Opaque => {
                            current_pipeline = Pipeline::Opaque;
                            render_pass.set_pipeline(&self.opaque_render_pipeline);
                        }
                        Pipeline::Transparent => {
                            current_pipeline = Pipeline::Transparent;
                            render_pass.set_pipeline(&self.transparent_render_pipeline);
                        }
                        Pipeline::Subpixel => {
                            current_pipeline = Pipeline::Subpixel;
                        }
                    },
                }
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

const DEPTH_TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

fn create_depth_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: DEPTH_TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (depth_texture, depth_view)
}
