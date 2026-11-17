//! Host display for the guest framebuffer.
//!
//! The window and GPU run on the main thread, while the guest CPU runs on its own
//! thread. Every refresh tick the display copies the rows marked dirty by
//! `SharedScreenBuffer` into a BGRA upload buffer and samples that texture into the
//! window.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle},
    window::{Window, WindowId},
};

use crate::simple_framebuffer::SharedScreenBuffer;

//15 Hz.
const REFRESH_INTERVAL: Duration = Duration::from_micros(1_000_000 / 15);
const WINDOW_TITLE: &str = "rv-emu-rs framebuffer";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayExit {
    EmulatorFinished,
    WindowClosed,
    //The display could not be initialized, or the event loop failed. The emulator
    //continues headless in that case.
    InitFailed,
}

pub fn run(screen: Arc<SharedScreenBuffer>, emulator_done: Arc<AtomicBool>) -> DisplayExit {
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(err) => {
            eprintln!("display: could not create event loop: {err}");
            return DisplayExit::InitFailed;
        }
    };
    let mut app = DisplayApp {
        screen,
        state: None,
        next_frame: Instant::now(),
        emulator_done,
        exit_reason: None,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("display: event loop failed: {err}");
    }
    app.exit_reason.unwrap_or(DisplayExit::InitFailed)
}

struct DisplayApp {
    screen: Arc<SharedScreenBuffer>,
    state: Option<GfxState>,
    next_frame: Instant,
    emulator_done: Arc<AtomicBool>,
    exit_reason: Option<DisplayExit>,
}

impl ApplicationHandler for DisplayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        if self.emulator_done.load(Ordering::Acquire) {
            self.exit_reason = Some(DisplayExit::EmulatorFinished);
            event_loop.exit();
            return;
        }
        let params = self.screen.params();
        let attributes = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(params.width as f64, params.height as f64));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                eprintln!("display: could not create window: {err}");
                self.exit_reason = Some(DisplayExit::InitFailed);
                event_loop.exit();
                return;
            }
        };
        match GfxState::new(
            window,
            event_loop.owned_display_handle(),
            self.screen.clone(),
        ) {
            Ok(state) => {
                self.state = Some(state);
                self.next_frame = Instant::now();
            }
            Err(err) => {
                eprintln!("display: {err}");
                self.exit_reason = Some(DisplayExit::InitFailed);
                event_loop.exit();
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.exit_reason = Some(DisplayExit::WindowClosed);
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    state.render();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.emulator_done.load(Ordering::Acquire) {
            self.exit_reason = Some(DisplayExit::EmulatorFinished);
            event_loop.exit();
            return;
        }
        let now = Instant::now();
        if now >= self.next_frame {
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
            self.next_frame = now + REFRESH_INTERVAL;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

struct GfxState {
    window: Arc<Window>,
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    texture: wgpu::Texture,
    texture_width: u32,
    texture_height: u32,
    frame: Vec<u8>,
    screen: Arc<SharedScreenBuffer>,
}

impl GfxState {
    fn new(
        window: Arc<Window>,
        display_handle: OwnedDisplayHandle,
        screen: Arc<SharedScreenBuffer>,
    ) -> Result<Self, String> {
        let params = screen.params();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(display_handle),
        ));
        let surface = instance
            .create_surface(window.clone())
            .map_err(|err| format!("could not create surface: {err}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|err| format!("could not find a GPU adapter: {err}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("rv-emu-rs display"),
            ..Default::default()
        }))
        .map_err(|err| format!("could not create a GPU device: {err}"))?;

        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| "surface has no supported configuration".to_string())?;
        //The guest's pixels are sRGB values, so prefer a non-sRGB swap chain format to
        //avoid an extra encode on presentation.
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(format) = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| !format.is_srgb())
        {
            config.format = format;
        }
        if capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Opaque)
        {
            config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        }
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("framebuffer texture"),
            size: wgpu::Extent3d {
                width: params.width,
                height: params.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("framebuffer sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("framebuffer bind group layout"),
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("framebuffer bind group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("framebuffer shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("framebuffer.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("framebuffer pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("framebuffer pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let frame = vec![0u8; (params.width * params.height * 4) as usize];
        Ok(GfxState {
            window,
            _instance: instance,
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            texture,
            texture_width: params.width,
            texture_height: params.height,
            frame,
            screen,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    fn update_frame(&mut self) {
        let width = self.texture_width as usize;
        for (row_index, row) in self.screen.rows().iter().enumerate() {
            let Some(content) = row.take_if_dirty() else {
                continue;
            };
            let row_start = row_index * width * 4;
            for (x, &pixel) in content.iter().take(width).enumerate() {
                let offset = row_start + x * 4;
                //Bgra8Unorm: blue, green, red, alpha. Simple-framebuffer alpha is
                //unused, so force the output opaque.
                self.frame[offset] = pixel as u8;
                self.frame[offset + 1] = (pixel >> 8) as u8;
                self.frame[offset + 2] = (pixel >> 16) as u8;
                self.frame[offset + 3] = 0xff;
            }
        }
    }

    fn render(&mut self) {
        self.update_frame();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &self.frame,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.texture_width * 4),
                rows_per_image: Some(self.texture_height),
            },
            wgpu::Extent3d {
                width: self.texture_width,
                height: self.texture_height,
                depth_or_array_layers: 1,
            },
        );
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("framebuffer encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("framebuffer pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
    }
}
