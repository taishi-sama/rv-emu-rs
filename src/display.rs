//! Host display for the guest framebuffer.
//!
//! The window and GPU run on the main thread, while the guest CPU runs on its own
//! thread. Every refresh tick the display copies the rows marked dirty by
//! `SharedScreenBuffer` into a BGRA upload buffer and samples that texture into the
//! window.

use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc::Sender,
    },
    time::{Duration, Instant},
};

use winit::{
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

use crate::{
    goldfish_events::{BTN_EXTRA, BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, BTN_SIDE, HostInput},
    simple_framebuffer::SharedScreenBuffer,
};

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

pub fn run(
    screen: Arc<SharedScreenBuffer>,
    emulator_done: Arc<AtomicBool>,
    instruction_counter: Arc<AtomicU32>,
    input: Sender<HostInput>,
) -> DisplayExit {
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
        instruction_counter,
        last_rate_update: Instant::now(),
        exit_reason: None,
        input,
        pressed_keys: HashSet::new(),
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
    instruction_counter: Arc<AtomicU32>,
    last_rate_update: Instant,
    exit_reason: Option<DisplayExit>,
    input: Sender<HostInput>,
    //Keys currently held so a focus loss can release them; a stuck key would
    //keep Doom moving or firing after the window loses focus.
    pressed_keys: HashSet<u16>,
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
                self.instruction_counter.swap(0, Ordering::Relaxed);
                self.last_rate_update = Instant::now();
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
            WindowEvent::KeyboardInput { event, .. } => {
                if event.repeat {
                    return;
                }
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let Some(code) = linux_keycode(code) else {
                    return;
                };
                self.set_key(code, event.state == ElementState::Pressed);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(code) = mouse_button_code(button) else {
                    return;
                };
                self.set_key(code, state == ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = &self.state {
                    if let Some((x, y)) = framebuffer_position(state, position) {
                        let _ = self.input.send(HostInput::Pointer { x, y });
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                //A button released outside the window may never produce a
                //MouseInput event without a pointer grab. Release held pointer
                //buttons but leave keyboard keys alone.
                let buttons: Vec<u16> = self
                    .pressed_keys
                    .iter()
                    .copied()
                    .filter(|&code| code >= BTN_LEFT)
                    .collect();
                for code in buttons {
                    self.set_key(code, false);
                }
            }
            WindowEvent::Focused(false) => {
                let pressed: Vec<u16> = self.pressed_keys.drain().collect();
                for code in pressed {
                    let _ = self.input.send(HostInput::Key {
                        code,
                        pressed: false,
                    });
                }
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
        if self.state.is_some()
            && now.duration_since(self.last_rate_update) >= Duration::from_secs(2)
        {
            let count = self.instruction_counter.swap(0, Ordering::Relaxed);
            let elapsed = now.duration_since(self.last_rate_update).as_secs_f64();
            let rate = if elapsed > 0.0 {
                (count as f64 / elapsed).round() as u64
            } else {
                0
            };
            if let Some(state) = &self.state {
                state
                    .window
                    .set_title(&format!("{WINDOW_TITLE} — {}", humanize_rate(rate)));
            }
            self.last_rate_update = now;
        }
        if now >= self.next_frame {
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
            self.next_frame = now + REFRESH_INTERVAL;
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

impl DisplayApp {
    fn set_key(&mut self, code: u16, pressed: bool) {
        if pressed {
            self.pressed_keys.insert(code);
        } else {
            self.pressed_keys.remove(&code);
        }
        let _ = self.input.send(HostInput::Key { code, pressed });
    }
}

//Maps a host cursor position, in window pixels, onto the guest framebuffer.
//The display stretches the framebuffer to the whole window, so this is a plain
//scale rather than a letterbox calculation.
fn framebuffer_position(state: &GfxState, position: PhysicalPosition<f64>) -> Option<(u16, u16)> {
    if !position.x.is_finite() || !position.y.is_finite() {
        return None;
    }
    let size = state.window.inner_size();
    if size.width == 0 || size.height == 0 {
        return None;
    }
    let params = state.screen.params();
    let x = (position.x.clamp(0.0, f64::from(size.width)) / f64::from(size.width)
        * f64::from(params.width)) as u32;
    let y = (position.y.clamp(0.0, f64::from(size.height)) / f64::from(size.height)
        * f64::from(params.height)) as u32;
    Some((
        x.min(params.width - 1) as u16,
        y.min(params.height - 1) as u16,
    ))
}

fn mouse_button_code(button: MouseButton) -> Option<u16> {
    match button {
        MouseButton::Left => Some(BTN_LEFT),
        MouseButton::Right => Some(BTN_RIGHT),
        MouseButton::Middle => Some(BTN_MIDDLE),
        MouseButton::Back => Some(BTN_SIDE),
        MouseButton::Forward => Some(BTN_EXTRA),
        _ => None,
    }
}

//Physical key codes make the mapping independent of the host keyboard layout,
//which is what Doom's default WASD/arrow bindings expect. Codes are Linux
//KEY_* values from input-event-codes.h.
fn linux_keycode(code: KeyCode) -> Option<u16> {
    use KeyCode::*;
    Some(match code {
        Escape => 1,
        Digit1 => 2,
        Digit2 => 3,
        Digit3 => 4,
        Digit4 => 5,
        Digit5 => 6,
        Digit6 => 7,
        Digit7 => 8,
        Digit8 => 9,
        Digit9 => 10,
        Digit0 => 11,
        Minus => 12,
        Equal => 13,
        Backspace => 14,
        Tab => 15,
        KeyQ => 16,
        KeyW => 17,
        KeyE => 18,
        KeyR => 19,
        KeyT => 20,
        KeyY => 21,
        KeyU => 22,
        KeyI => 23,
        KeyO => 24,
        KeyP => 25,
        BracketLeft => 26,
        BracketRight => 27,
        Enter => 28,
        ControlLeft => 29,
        KeyA => 30,
        KeyS => 31,
        KeyD => 32,
        KeyF => 33,
        KeyG => 34,
        KeyH => 35,
        KeyJ => 36,
        KeyK => 37,
        KeyL => 38,
        Semicolon => 39,
        Quote => 40,
        Backquote => 41,
        ShiftLeft => 42,
        Backslash => 43,
        KeyZ => 44,
        KeyX => 45,
        KeyC => 46,
        KeyV => 47,
        KeyB => 48,
        KeyN => 49,
        KeyM => 50,
        Comma => 51,
        Period => 52,
        Slash => 53,
        ShiftRight => 54,
        NumpadMultiply => 55,
        AltLeft => 56,
        Space => 57,
        CapsLock => 58,
        F1 => 59,
        F2 => 60,
        F3 => 61,
        F4 => 62,
        F5 => 63,
        F6 => 64,
        F7 => 65,
        F8 => 66,
        F9 => 67,
        F10 => 68,
        NumLock => 69,
        ScrollLock => 70,
        Numpad7 => 71,
        Numpad8 => 72,
        Numpad9 => 73,
        NumpadSubtract => 74,
        Numpad4 => 75,
        Numpad5 => 76,
        Numpad6 => 77,
        NumpadAdd => 78,
        Numpad1 => 79,
        Numpad2 => 80,
        Numpad3 => 81,
        Numpad0 => 82,
        NumpadDecimal => 83,
        IntlBackslash => 86,
        F11 => 87,
        F12 => 88,
        NumpadEnter => 96,
        ControlRight => 97,
        NumpadDivide => 98,
        PrintScreen => 99,
        AltRight => 100,
        Home => 102,
        ArrowUp => 103,
        PageUp => 104,
        ArrowLeft => 105,
        ArrowRight => 106,
        End => 107,
        ArrowDown => 108,
        PageDown => 109,
        Insert => 110,
        Delete => 111,
        Pause => 119,
        SuperLeft => 125,
        SuperRight => 126,
        ContextMenu => 127,
        _ => return None,
    })
}

fn humanize_rate(rate: u64) -> String {
    if rate >= 1_000_000 {
        format!("{:.2} MIPS", rate as f64 / 1_000_000.0)
    } else if rate >= 1_000 {
        format!("{:.2} KIPS", rate as f64 / 1_000.0)
    } else {
        format!("{rate} IPS")
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
        let adapter_info = adapter.get_info();
        eprintln!(
            "display: using {} ({:?})",
            adapter_info.name, adapter_info.backend
        );
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

#[cfg(test)]
mod tests {
    use super::humanize_rate;

    #[test]
    fn humanizes_instruction_rates() {
        assert_eq!(humanize_rate(0), "0 IPS");
        assert_eq!(humanize_rate(999), "999 IPS");
        assert_eq!(humanize_rate(1_000), "1.00 KIPS");
        assert_eq!(humanize_rate(12_345), "12.35 KIPS");
        assert_eq!(humanize_rate(1_234_567), "1.23 MIPS");
        assert_eq!(humanize_rate(12_345_678), "12.35 MIPS");
    }

    #[test]
    fn maps_doom_keys_to_linux_codes() {
        use winit::keyboard::KeyCode;

        assert_eq!(super::linux_keycode(KeyCode::KeyW), Some(17));
        assert_eq!(super::linux_keycode(KeyCode::KeyA), Some(30));
        assert_eq!(super::linux_keycode(KeyCode::Space), Some(57));
        assert_eq!(super::linux_keycode(KeyCode::ControlLeft), Some(29));
        assert_eq!(super::linux_keycode(KeyCode::ShiftRight), Some(54));
        assert_eq!(super::linux_keycode(KeyCode::ArrowUp), Some(103));
        assert_eq!(super::linux_keycode(KeyCode::F12), Some(88));
        assert_eq!(super::linux_keycode(KeyCode::F13), None);
    }
}
