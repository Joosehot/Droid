//! wgpu renderer: instanced primitive meshes + screen-space UI quads (bitmap text).

use crate::font;
use crate::spec::{Shape, View};
use crate::world::{Transform, Visual, World};
use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    /// Atlas rect u0, v0, u1, v1. All zero = untextured.
    uv_rect: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    /// x = 1.0 for lit (3D), 0.0 for unlit (2D)
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UiQuad {
    /// NDC rect: x, y (top-left), w, h
    rect: [f32; 4],
    color: [f32; 4],
}

/// Screen-space extras drawn on top of the game (chat box, status bar).
#[derive(Default)]
pub struct Overlay {
    /// (x, y, w, h, rgba) in pixels from top-left.
    pub rects: Vec<(f32, f32, f32, f32, [f32; 4])>,
    /// (text, x, y, size, rgb) in pixels from top-left.
    pub texts: Vec<(String, f32, f32, f32, [f32; 3])>,
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    instances: wgpu::Buffer,
    instance_capacity: usize,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth: wgpu::TextureView,
    scene_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    /// 0 = box, 1 = sphere, 2 = plane, then one per procedural model.
    meshes: Vec<Mesh>,
    model_gen: u64,
    ui_buffer: wgpu::Buffer,
    ui_capacity: usize,
    atlas_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    atlas: Atlas,
    pub window: Arc<Window>,
}

/// All sprites of the current game packed into one texture.
struct Atlas {
    bind_group: wgpu::BindGroup,
    /// Per sprite index: u0, v0, u1, v1.
    rects: Vec<[f32; 4]>,
    gen: u64,
}

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> g: Globals;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_smp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(9) vcolor: vec3<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
    @location(7) color: vec4<f32>,
    @location(8) uv_rect: vec4<f32>,
};
struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) textured: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.m0, in.m1, in.m2, in.m3);
    var out: VsOut;
    out.clip = g.view_proj * model * vec4<f32>(in.pos, 1.0);
    let n = (model * vec4<f32>(in.normal, 0.0)).xyz;
    out.normal = normalize(n);
    out.color = vec4<f32>(in.color.rgb * in.vcolor, in.color.a);
    out.uv = mix(in.uv_rect.xy, in.uv_rect.zw, in.uv);
    out.textured = select(0.0, 1.0, in.uv_rect.z != 0.0 || in.uv_rect.x != 0.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var base = in.color.rgb;
    if (in.textured > 0.5) {
        let t = textureSample(atlas_tex, atlas_smp, in.uv);
        if (t.a < 0.5) { discard; }
        base = base * t.rgb;
    }
    if (g.params.x < 0.5 || in.textured > 0.5) {
        return vec4<f32>(base, 1.0);
    }
    let l = normalize(g.light_dir.xyz);
    let diff = max(dot(normalize(in.normal), l), 0.0);
    let shade = 0.35 + 0.65 * diff;
    return vec4<f32>(base * shade, 1.0);
}

struct UiIn {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
};
struct UiOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn ui_vs(@builtin(vertex_index) vi: u32, in: UiIn) -> UiOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 1.0));
    let c = corners[vi];
    let x = in.rect.x + c.x * in.rect.z;
    let y = in.rect.y - c.y * in.rect.w;
    var out: UiOut;
    out.clip = vec4<f32>(x, y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn ui_fs(in: UiOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

impl Renderer {
    pub fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow::anyhow!("no compatible GPU adapter"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("aige"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                memory_hints: Default::default(),
            },
            None,
        ))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let depth = create_depth(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("aige shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals bind"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });

        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
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
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let atlas = build_atlas(&device, &queue, &atlas_layout, &sampler, &[], 0);

        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene layout"),
            bind_group_layouts: &[&globals_layout, &atlas_layout],
            push_constant_ranges: &[],
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 9 => Float32x3],
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4],
        };
        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene"),
            layout: Some(&scene_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout, instance_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let ui_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui"),
            layout: Some(&ui_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("ui_vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiQuad>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("ui_fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let meshes = vec![
            make_mesh(&device, &cube()),
            make_mesh(&device, &sphere(16, 24)),
            make_mesh(&device, &plane()),
        ];
        let ui_capacity = 4096;
        let ui_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui quads"),
            size: (ui_capacity * std::mem::size_of::<UiQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            depth,
            scene_pipeline,
            ui_pipeline,
            globals,
            globals_bind,
            meshes,
            model_gen: 0,
            ui_buffer,
            ui_capacity,
            atlas_layout,
            sampler,
            atlas,
            window,
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
        self.depth = create_depth(&self.device, &self.config);
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    pub fn render(&mut self, world: &World, overlay: &Overlay) {
        if self.model_gen != world.model_gen {
            self.meshes.truncate(3);
            for (name, m) in &world.models {
                let t = std::time::Instant::now();
                let data = crate::models::build(m);
                log::info!("built model '{name}': {} verts, {} tris in {:.0} ms", data.positions.len(), data.indices.len() / 3, t.elapsed().as_secs_f32() * 1000.0);
                let verts: Vec<Vertex> = data
                    .positions
                    .iter()
                    .zip(&data.normals)
                    .zip(&data.colors)
                    .map(|((p, n), c)| Vertex { pos: *p, normal: *n, uv: [0.0, 0.0], color: *c })
                    .collect();
                self.meshes.push(make_mesh(&self.device, &(verts, data.indices)));
            }
            self.model_gen = world.model_gen;
        }
        if self.atlas.gen != world.sprite_gen {
            self.atlas = build_atlas(&self.device, &self.queue, &self.atlas_layout, &self.sampler, &world.sprites, world.sprite_gen);
        }
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            Err(e) => {
                log::warn!("surface error: {e:?}");
                return;
            }
        };
        let view = frame.texture.create_view(&Default::default());

        // Camera.
        let aspect = self.aspect();
        let cam = &world.camera;
        let view_proj = match world.view {
            View::TwoD => {
                let h = cam.zoom;
                let w = h * aspect;
                let proj = Mat4::orthographic_rh(-w * 0.5, w * 0.5, -h * 0.5, h * 0.5, -100.0, 100.0);
                let v = Mat4::from_translation(Vec3::new(-cam.position.x, -cam.position.y, 0.0));
                proj * v
            }
            View::ThreeD => {
                let proj = Mat4::perspective_rh(60f32.to_radians(), aspect, 0.1, 500.0);
                let up = if (cam.position - cam.look_at).normalize_or_zero().abs().y > 0.99 { Vec3::Z } else { Vec3::Y };
                let v = Mat4::look_at_rh(cam.position, cam.look_at, up);
                proj * v
            }
        };
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            light_dir: Vec4::new(0.4, 1.0, 0.6, 0.0).to_array(),
            params: [if world.view == View::ThreeD { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
        };
        self.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        // Instances grouped by shape.
        let mut groups: Vec<Vec<Instance>> = vec![Vec::new(); self.meshes.len()];
        for (_, (t, v)) in world.ecs.query::<(&Transform, &Visual)>().iter() {
            let rot = glam::Quat::from_euler(
                glam::EulerRot::XYZ,
                t.rot.x.to_radians(),
                t.rot.y.to_radians(),
                t.rot.z.to_radians(),
            );
            let model = Mat4::from_scale_rotation_translation(t.scale, rot, t.pos);
            let idx = match v.model {
                Some(m) if 3 + m < self.meshes.len() => 3 + m,
                _ => match v.shape {
                    Shape::Box => 0,
                    Shape::Sphere => 1,
                    Shape::Plane => 2,
                },
            };
            let mut uv_rect = [0.0; 4];
            if let Some(r) = v.sprite.and_then(|i| self.atlas.rects.get(i)) {
                uv_rect = *r;
                if v.flip_x {
                    uv_rect.swap(0, 2);
                }
            }
            groups[idx].push(Instance { model: model.to_cols_array_2d(), color: [v.color.x, v.color.y, v.color.z, 1.0], uv_rect });
        }
        for (i, g) in groups.iter().enumerate() {
            let m = &mut self.meshes[i];
            if g.len() > m.instance_capacity {
                m.instance_capacity = g.len().next_power_of_two();
                m.instances = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("instances"),
                    size: (m.instance_capacity * std::mem::size_of::<Instance>()) as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
            }
            if !g.is_empty() {
                self.queue.write_buffer(&m.instances, 0, bytemuck::cast_slice(g));
            }
        }

        // UI text quads.
        let mut quads: Vec<UiQuad> = Vec::new();
        let (sw, sh) = (self.config.width as f32, self.config.height as f32);
        let mut texts: Vec<(&str, f32, f32, f32, [f32; 3])> = world
            .texts
            .iter()
            .map(|t| (t.text.as_str(), t.x, t.y, t.size, t.color))
            .collect();
        if let Some(e) = &world.error {
            texts.push((e.as_str(), 0.02, 0.9, 16.0, [1.0, 0.3, 0.3]));
        }
        let to_ndc = |px: f32, py: f32, w: f32, h: f32| [px / sw * 2.0 - 1.0, 1.0 - py / sh * 2.0, w / sw * 2.0, h / sh * 2.0];
        for (text, x, y, size, color) in texts {
            font::layout(text, x * sw, y * sh, size, |px, py, w, h| {
                quads.push(UiQuad { rect: to_ndc(px, py, w, h), color: [color[0], color[1], color[2], 1.0] });
            });
        }
        for (x, y, w, h, color) in &overlay.rects {
            quads.push(UiQuad { rect: to_ndc(*x, *y, *w, *h), color: *color });
        }
        for (text, x, y, size, color) in &overlay.texts {
            font::layout(text, *x, *y, *size, |px, py, w, h| {
                quads.push(UiQuad { rect: to_ndc(px, py, w, h), color: [color[0], color[1], color[2], 1.0] });
            });
        }
        if quads.len() > self.ui_capacity {
            self.ui_capacity = quads.len().next_power_of_two();
            self.ui_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("ui quads"),
                size: (self.ui_capacity * std::mem::size_of::<UiQuad>()) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !quads.is_empty() {
            self.queue.write_buffer(&self.ui_buffer, 0, bytemuck::cast_slice(&quads));
        }

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let bg = world.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: srgb_to_linear(bg.x) as f64,
                            g: srgb_to_linear(bg.y) as f64,
                            b: srgb_to_linear(bg.z) as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_bind_group(1, &self.atlas.bind_group, &[]);
            for (i, g) in groups.iter().enumerate() {
                if g.is_empty() {
                    continue;
                }
                let m = &self.meshes[i];
                pass.set_vertex_buffer(0, m.vertices.slice(..));
                pass.set_vertex_buffer(1, m.instances.slice(..));
                pass.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.index_count, 0, 0..g.len() as u32);
            }
            if !quads.is_empty() {
                pass.set_pipeline(&self.ui_pipeline);
                pass.set_vertex_buffer(0, self.ui_buffer.slice(..));
                pass.draw(0..6, 0..quads.len() as u32);
            }
        }
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn create_depth(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
        size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&Default::default())
}

fn make_mesh(device: &wgpu::Device, (verts, idx): &(Vec<Vertex>, Vec<u32>)) -> Mesh {
    let empty_v = [Vertex { pos: [0.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.0; 2], color: [1.0; 3] }];
    let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mesh vertices"),
        contents: if verts.is_empty() { bytemuck::cast_slice(&empty_v) } else { bytemuck::cast_slice(verts) },
        usage: wgpu::BufferUsages::VERTEX,
    });
    let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("mesh indices"),
        contents: if idx.is_empty() { bytemuck::cast_slice(&[0u32; 4]) } else { bytemuck::cast_slice(idx) },
        usage: wgpu::BufferUsages::INDEX,
    });
    let instance_capacity = 256;
    let instances = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instances"),
        size: (instance_capacity * std::mem::size_of::<Instance>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    Mesh { vertices, indices, index_count: idx.len() as u32, instances, instance_capacity }
}

fn cube() -> (Vec<Vertex>, Vec<u32>) {
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        ([0.0, 0.0, 1.0], [[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]]),
        ([0.0, 0.0, -1.0], [[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5]]),
        ([1.0, 0.0, 0.0], [[0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5]]),
        ([-1.0, 0.0, 0.0], [[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5]]),
        ([0.0, 1.0, 0.0], [[-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5]]),
        ([0.0, -1.0, 0.0], [[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]]),
    ];
    let mut v = Vec::new();
    let mut i = Vec::new();
    for (n, quad) in faces {
        let base = v.len() as u32;
        for p in quad {
            v.push(Vertex { pos: p, normal: n, uv: [0.0, 0.0], color: [1.0; 3] });
        }
        i.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (v, i)
}

fn plane() -> (Vec<Vertex>, Vec<u32>) {
    let n = [0.0, 0.0, 1.0];
    // uv y grows downward (top row of the sprite = v 0).
    let v = vec![
        Vertex { pos: [-0.5, -0.5, 0.0], normal: n, uv: [0.0, 1.0], color: [1.0; 3] },
        Vertex { pos: [0.5, -0.5, 0.0], normal: n, uv: [1.0, 1.0], color: [1.0; 3] },
        Vertex { pos: [0.5, 0.5, 0.0], normal: n, uv: [1.0, 0.0], color: [1.0; 3] },
        Vertex { pos: [-0.5, 0.5, 0.0], normal: n, uv: [0.0, 0.0], color: [1.0; 3] },
    ];
    (v, vec![0, 1, 2, 0, 2, 3])
}

fn sphere(rings: u32, segments: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut v = Vec::new();
    let mut i = Vec::new();
    for r in 0..=rings {
        let phi = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..=segments {
            let theta = 2.0 * std::f32::consts::PI * s as f32 / segments as f32;
            let n = [phi.sin() * theta.cos(), phi.cos(), phi.sin() * theta.sin()];
            v.push(Vertex { pos: [n[0] * 0.5, n[1] * 0.5, n[2] * 0.5], normal: n, uv: [0.0, 0.0], color: [1.0; 3] });
        }
    }
    let stride = segments + 1;
    for r in 0..rings {
        for s in 0..segments {
            let a = r * stride + s;
            let b = a + stride;
            i.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    (v, i)
}

/// Packs sprites into one RGBA texture using simple shelf packing.
fn build_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    sprites: &[(String, crate::spec::SpriteSpec)],
    gen: u64,
) -> Atlas {
    const PAD: u32 = 1;
    let sizes: Vec<(u32, u32)> = sprites.iter().map(|(_, s)| s.size()).collect();
    let max_w: u32 = 1024;
    // Shelf packing.
    let mut places: Vec<(u32, u32)> = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut shelf_h) = (PAD, PAD, 0u32);
    let mut used_w = 1u32;
    for &(w, h) in &sizes {
        if x + w + PAD > max_w && x > PAD {
            x = PAD;
            y += shelf_h + PAD;
            shelf_h = 0;
        }
        places.push((x, y));
        x += w + PAD;
        shelf_h = shelf_h.max(h);
        used_w = used_w.max(x);
    }
    let atlas_w = used_w.max(1);
    let atlas_h = (y + shelf_h + PAD).max(1);

    let mut pixels = vec![0u8; (atlas_w * atlas_h * 4) as usize];
    let mut rects = Vec::with_capacity(sizes.len());
    for (i, (_, spr)) in sprites.iter().enumerate() {
        let (w, h) = sizes[i];
        let (px, py) = places[i];
        let data = spr.rgba();
        for row in 0..h {
            let src = (row * w * 4) as usize;
            let dst = (((py + row) * atlas_w + px) * 4) as usize;
            pixels[dst..dst + (w * 4) as usize].copy_from_slice(&data[src..src + (w * 4) as usize]);
        }
        rects.push([
            px as f32 / atlas_w as f32,
            py as f32 / atlas_h as f32,
            (px + w) as f32 / atlas_w as f32,
            (py + h) as f32 / atlas_h as f32,
        ]);
    }

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("sprite atlas"),
        size: wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &pixels,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(atlas_w * 4), rows_per_image: Some(atlas_h) },
        wgpu::Extent3d { width: atlas_w, height: atlas_h, depth_or_array_layers: 1 },
    );
    let view = texture.create_view(&Default::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("atlas bind"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    });
    Atlas { bind_group, rects, gen }
}
