mod plugin;
mod render;
mod spaces;
mod text;
mod theme;

use anyhow::Result;
use vello::kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Fill, FontData};
use vello::util::RenderContext;
use vello::wgpu;
use vello::{AaConfig, Renderer, Scene};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::ModifiersState;

use render::RenderState;
use spaces::{BuiltInSpace, LoadedSpace};
use theme::*;

const SPACE_BAR_W: f64 = 40.0;

struct ConstructApp {
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    state: RenderState,
    scene: Scene,
    scale_factor: f64,
    font: Option<FontData>,
    mono_font: Option<FontData>,
    modifiers: ModifiersState,
    mouse_pos: (f64, f64),
    window_size: (f64, f64),

    // Spaces
    spaces: Vec<LoadedSpace>,
    active_idx: usize,
}

impl ApplicationHandler for ConstructApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let RenderState::Suspended(cached_window) = &mut self.state else {
            return;
        };

        let window = cached_window
            .take()
            .unwrap_or_else(|| render::create_window(event_loop));

        self.scale_factor = window.scale_factor();

        let size = window.inner_size();
        self.window_size = (
            size.width as f64 / self.scale_factor,
            size.height as f64 / self.scale_factor,
        );

        let surface_future = self.context.create_surface(
            window.clone(),
            size.width,
            size.height,
            wgpu::PresentMode::AutoVsync,
        );
        let surface = pollster::block_on(surface_future).expect("Error creating surface");

        self.renderers
            .resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id]
            .get_or_insert_with(|| render::create_renderer(&self.context, &surface));

        window.request_redraw();

        self.state = RenderState::Active {
            surface: Box::new(surface),
            valid_surface: true,
            window,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        if let RenderState::Active { window, .. } = &self.state {
            self.state = RenderState::Suspended(Some(window.clone()));
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        // Handle RedrawRequested separately to avoid borrow conflicts
        if matches!(event, WindowEvent::RedrawRequested) {
            self.handle_redraw();
            return;
        }

        let (surface, valid_surface, window) = match &mut self.state {
            RenderState::Active {
                surface,
                valid_surface,
                window,
            } if window.id() == window_id => (surface, valid_surface, window.clone()),
            _ => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                if size.width != 0 && size.height != 0 {
                    self.context.resize_surface(surface, size.width, size.height);
                    self.window_size = (
                        size.width as f64 / self.scale_factor,
                        size.height as f64 / self.scale_factor,
                    );
                    *valid_surface = true;
                } else {
                    *valid_surface = false;
                }
                window.request_redraw();
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = scale_factor;
            }

            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }

            WindowEvent::CursorMoved { position, .. } => {
                let sf = self.scale_factor;
                let mx = position.x / sf;
                let my = position.y / sf;
                self.mouse_pos = (mx, my);

                let area = Rect::new(SPACE_BAR_W, 0.0, self.window_size.0, self.window_size.1);
                if mx >= SPACE_BAR_W {
                    let space = self.spaces[self.active_idx].as_space_mut();
                    if space.handle_mouse_move(mx, my, area) {
                        window.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput {
                state: btn_state,
                button,
                ..
            } => {
                let (mx, my) = self.mouse_pos;
                let area = Rect::new(SPACE_BAR_W, 0.0, self.window_size.0, self.window_size.1);

                match btn_state {
                    ElementState::Pressed => {
                        // Click in space bar?
                        if mx < SPACE_BAR_W {
                            for i in 0..self.spaces.len() {
                                let y = 12.0 + i as f64 * 44.0;
                                if my >= y && my < y + 32.0 {
                                    self.active_idx = i;
                                    break;
                                }
                            }
                        } else {
                            let space = self.spaces[self.active_idx].as_space_mut();
                            space.handle_mouse_click(mx, my, button, area);
                        }
                    }
                    ElementState::Released => {
                        if mx >= SPACE_BAR_W {
                            let space = self.spaces[self.active_idx].as_space_mut();
                            space.handle_mouse_release(mx, my, button, area);
                        }
                    }
                }
                window.request_redraw();
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (mx, my) = self.mouse_pos;
                if mx >= SPACE_BAR_W {
                    let area = Rect::new(SPACE_BAR_W, 0.0, self.window_size.0, self.window_size.1);
                    let (dx, dy) = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x as f64 * 20.0, y as f64 * 20.0),
                        MouseScrollDelta::PixelDelta(p) => (p.x / self.scale_factor, p.y / self.scale_factor),
                    };
                    let space = self.spaces[self.active_idx].as_space_mut();
                    space.handle_scroll(dx, dy, mx, my, area);
                    window.request_redraw();
                }
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }

                let space = self.spaces[self.active_idx].as_space_mut();
                let handled = space.handle_key(&event.logical_key, self.modifiers);

                // If the space didn't handle it and there's text input, try handle_char
                if !handled {
                    if let Some(ref txt) = event.text {
                        let cmd = self.modifiers.super_key() || self.modifiers.control_key();
                        if !cmd {
                            let space = self.spaces[self.active_idx].as_space_mut();
                            space.handle_char(txt.as_str(), self.modifiers);
                        }
                    }
                }

                window.request_redraw();
            }

            _ => {}
        }
    }
}

impl ConstructApp {
    fn handle_redraw(&mut self) {
        let RenderState::Active {
            surface,
            valid_surface,
            window,
        } = &mut self.state
        else {
            return;
        };
        if !*valid_surface {
            return;
        }

        self.scene.reset();
        let sf = self.scale_factor;
        let xf = Affine::scale(sf);
        let ww = self.window_size.0;
        let wh = self.window_size.1;

        // Full background
        self.scene.fill(Fill::NonZero, xf, BG, None, &Rect::new(0.0, 0.0, ww, wh));

        // Space area
        let area = Rect::new(SPACE_BAR_W, 0.0, ww, wh);

        // Draw active space
        if let (Some(font), Some(mono_font)) = (&self.font, &self.mono_font) {
            let space = self.spaces[self.active_idx].as_space_mut();
            space.draw(&mut self.scene, xf, area, font, mono_font);
        }

        // Draw space bar on top
        self.scene.fill(Fill::NonZero, xf, SPACE_BAR_BG, None, &Rect::new(0.0, 0.0, SPACE_BAR_W, wh));
        self.scene.stroke(&Stroke::new(1.0), xf, SPACE_BAR_BORDER, None, &Line::new((SPACE_BAR_W - 0.5, 0.0), (SPACE_BAR_W - 0.5, wh)));

        for (i, space) in self.spaces.iter().enumerate() {
            let is_active = i == self.active_idx;
            let y = 12.0 + i as f64 * 44.0;
            let cx = SPACE_BAR_W / 2.0;
            let cy = y + 14.0;

            if y > wh { break; }

            if is_active {
                self.scene.fill(Fill::NonZero, xf, SPACE_BAR_ACTIVE, None, &RoundedRect::from_rect(Rect::new(4.0, y, SPACE_BAR_W - 4.0, y + 32.0), 6.0));
            }

            let icon_color = if is_active { SPACE_BAR_ICON_ACTIVE } else { SPACE_BAR_ICON };
            let s = &Stroke::new(1.5);

            match space.as_space().name() {
                "Design" => {
                    // Diamond
                    let mut path = vello::kurbo::BezPath::new();
                    path.move_to((cx, cy - 8.0));
                    path.line_to((cx + 7.0, cy));
                    path.line_to((cx, cy + 8.0));
                    path.line_to((cx - 7.0, cy));
                    path.close_path();
                    self.scene.stroke(s, xf, icon_color, None, &path);
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx, cy), 2.0));
                }
                "Code" => {
                    // Angle brackets < >
                    let mut left = vello::kurbo::BezPath::new();
                    left.move_to((cx - 1.0, cy - 7.0));
                    left.line_to((cx - 7.0, cy));
                    left.line_to((cx - 1.0, cy + 7.0));
                    self.scene.stroke(s, xf, icon_color, None, &left);
                    let mut right = vello::kurbo::BezPath::new();
                    right.move_to((cx + 1.0, cy - 7.0));
                    right.line_to((cx + 7.0, cy));
                    right.line_to((cx + 1.0, cy + 7.0));
                    self.scene.stroke(s, xf, icon_color, None, &right);
                }
                "Notes" => {
                    // Notepad lines
                    self.scene.stroke(s, xf, icon_color, None, &Rect::new(cx - 7.0, cy - 8.0, cx + 7.0, cy + 8.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 4.0, cy - 4.0), (cx + 4.0, cy - 4.0)));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 4.0, cy), (cx + 4.0, cy)));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 4.0, cy + 4.0), (cx + 2.0, cy + 4.0)));
                }
                "Paint" => {
                    // Brush/circle
                    self.scene.stroke(s, xf, icon_color, None, &Circle::new((cx, cy), 6.0));
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx, cy), 3.0));
                }
                "Terminal" => {
                    // Terminal prompt >_
                    self.scene.stroke(s, xf, icon_color, None, &Rect::new(cx - 8.0, cy - 7.0, cx + 8.0, cy + 7.0));
                    let mut prompt = vello::kurbo::BezPath::new();
                    prompt.move_to((cx - 4.0, cy - 3.0));
                    prompt.line_to((cx - 1.0, cy));
                    prompt.line_to((cx - 4.0, cy + 3.0));
                    self.scene.stroke(&Stroke::new(1.5), xf, icon_color, None, &prompt);
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx + 1.0, cy + 3.0), (cx + 5.0, cy + 3.0)));
                }
                "Calendar" => {
                    // Calendar grid
                    self.scene.stroke(s, xf, icon_color, None, &Rect::new(cx - 7.0, cy - 7.0, cx + 7.0, cy + 7.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 7.0, cy - 3.0), (cx + 7.0, cy - 3.0)));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx, cy - 3.0), (cx, cy + 7.0)));
                }
                "Kanban" => {
                    // Three columns
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Rect::new(cx - 7.0, cy - 7.0, cx - 2.0, cy + 3.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Rect::new(cx - 1.0, cy - 7.0, cx + 3.0, cy + 7.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Rect::new(cx + 4.0, cy - 7.0, cx + 8.0, cy));
                }
                "Chat" => {
                    // Speech bubble
                    self.scene.stroke(s, xf, icon_color, None, &RoundedRect::from_rect(Rect::new(cx - 8.0, cy - 6.0, cx + 8.0, cy + 4.0), 4.0));
                    let mut tail = vello::kurbo::BezPath::new();
                    tail.move_to((cx - 3.0, cy + 4.0));
                    tail.line_to((cx - 5.0, cy + 8.0));
                    tail.line_to((cx + 1.0, cy + 4.0));
                    self.scene.stroke(s, xf, icon_color, None, &tail);
                }
                "AI" => {
                    // Brain/star
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx, cy), 4.0));
                    self.scene.stroke(s, xf, icon_color, None, &Circle::new((cx, cy), 8.0));
                    // Rays
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx, cy - 8.0), (cx, cy - 11.0)));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx + 7.0, cy - 4.0), (cx + 10.0, cy - 6.0)));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 7.0, cy - 4.0), (cx - 10.0, cy - 6.0)));
                }
                "Docs" => {
                    // Book/page
                    self.scene.stroke(s, xf, icon_color, None, &Rect::new(cx - 7.0, cy - 8.0, cx + 7.0, cy + 8.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx, cy - 8.0), (cx, cy + 8.0)));
                }
                "Git" => {
                    // Git branch
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx, cy - 5.0), 2.5));
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx, cy + 5.0), 2.5));
                    self.scene.stroke(s, xf, icon_color, None, &Line::new((cx, cy - 2.5), (cx, cy + 2.5)));
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx + 5.0, cy - 2.0), 2.0));
                    let mut branch = vello::kurbo::BezPath::new();
                    branch.move_to((cx, cy + 2.0));
                    branch.curve_to((cx, cy - 2.0), (cx + 3.0, cy - 2.0), (cx + 5.0, cy - 4.0));
                    self.scene.stroke(s, xf, icon_color, None, &branch);
                }
                "Architect" => {
                    // Node diagram
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Rect::new(cx - 8.0, cy - 5.0, cx - 2.0, cy + 1.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Rect::new(cx + 2.0, cy - 8.0, cx + 8.0, cy - 2.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Rect::new(cx + 2.0, cy + 2.0, cx + 8.0, cy + 8.0));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 2.0, cy - 2.0), (cx + 2.0, cy - 5.0)));
                    self.scene.stroke(&Stroke::new(1.0), xf, icon_color, None, &Line::new((cx - 2.0, cy), (cx + 2.0, cy + 5.0)));
                }
                "Releases" => {
                    // Tag
                    self.scene.stroke(s, xf, icon_color, None, &RoundedRect::from_rect(Rect::new(cx - 6.0, cy - 5.0, cx + 7.0, cy + 5.0), 3.0));
                    self.scene.fill(Fill::NonZero, xf, icon_color, None, &Circle::new((cx - 3.0, cy), 1.5));
                }
                _ => {
                    self.scene.stroke(s, xf, icon_color, None, &Circle::new((cx, cy), 8.0));
                }
            }
        }

        // Render to surface
        let width = surface.config.width;
        let height = surface.config.height;
        let device_handle = &self.context.devices[surface.dev_id];

        self.renderers[surface.dev_id]
            .as_mut()
            .unwrap()
            .render_to_texture(
                &device_handle.device,
                &device_handle.queue,
                &self.scene,
                &surface.target_view,
                &vello::RenderParams {
                    base_color: BG,
                    width,
                    height,
                    antialiasing_method: AaConfig::Msaa16,
                },
            )
            .expect("failed to render");

        let surface_texture = surface
            .surface
            .get_current_texture()
            .expect("failed to get surface texture");

        let mut encoder =
            device_handle
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Surface Blit"),
                });
        surface.blitter.copy(
            &device_handle.device,
            &mut encoder,
            &surface.target_view,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
        );
        device_handle.queue.submit([encoder.finish()]);
        surface_texture.present();
        device_handle.device.poll(wgpu::PollType::Poll).unwrap();

        window.request_redraw();
    }
}

fn main() -> Result<()> {
    let font = text::load_font();
    let mono_font = text::load_mono_font();

    // Built-in spaces
    let mut all_spaces: Vec<LoadedSpace> = vec![
        LoadedSpace::BuiltIn(BuiltInSpace::Design(spaces::design::DesignSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Code(spaces::code::CodeSpace::new(mono_font.as_ref()))),
        LoadedSpace::BuiltIn(BuiltInSpace::Notes(spaces::notes::NotesSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Paint(spaces::paint::PaintSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Terminal(spaces::terminal::TerminalSpace::new(mono_font.as_ref()))),
        LoadedSpace::BuiltIn(BuiltInSpace::Calendar(spaces::calendar::CalendarSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Kanban(spaces::kanban::KanbanSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Chat(spaces::chat::ChatSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Ai(spaces::ai::AiSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Docs(spaces::docs::DocsSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Git(spaces::git::GitSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Architect(spaces::architect::ArchitectSpace::new())),
        LoadedSpace::BuiltIn(BuiltInSpace::Releases(spaces::releases::ReleasesSpace::new())),
    ];

    // Load plugin spaces from ~/.construct/spaces/
    let plugin_spaces = plugin::SpaceLoader::scan_plugin_spaces();
    all_spaces.extend(plugin_spaces);

    let mut app = ConstructApp {
        context: RenderContext::new(),
        renderers: vec![],
        state: RenderState::Suspended(None),
        scene: Scene::new(),
        scale_factor: 1.0,
        font,
        mono_font,
        modifiers: ModifiersState::empty(),
        mouse_pos: (0.0, 0.0),
        window_size: (1400.0, 900.0),
        spaces: all_spaces,
        active_idx: 0,
    };

    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}
