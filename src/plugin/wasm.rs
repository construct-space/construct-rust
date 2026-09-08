use std::path::Path;

use anyhow::{Context, Result};
use vello::kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use wasmtime::*;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::spaces::Space;
use crate::text::draw_text;

struct WasmState {
    scene: *mut Scene,
    xf: Affine,
    font: *const FontData,
    mono_font: *const FontData,
    memory: Option<Memory>,
}

// WasmState holds raw pointers that we set/unset around each call
unsafe impl Send for WasmState {}

pub struct WasmSpace {
    store: Store<WasmState>,
    instance: Instance,
    handle: i32,
    name: String,
}

fn read_wasm_string(store: &Store<WasmState>, ptr: i32, len: i32) -> String {
    let state = store.data();
    if let Some(memory) = state.memory {
        let data = memory.data(&store);
        let start = ptr as usize;
        let end = start + len as usize;
        if end <= data.len() {
            return String::from_utf8_lossy(&data[start..end]).into_owned();
        }
    }
    String::new()
}

impl WasmSpace {
    pub fn load(wasm_path: &Path) -> Result<Self> {
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_path)
            .with_context(|| format!("Failed to load WASM module: {}", wasm_path.display()))?;

        let mut store = Store::new(
            &engine,
            WasmState {
                scene: std::ptr::null_mut(),
                xf: Affine::IDENTITY,
                font: std::ptr::null(),
                mono_font: std::ptr::null(),
                memory: None,
            },
        );

        let mut linker = Linker::new(&engine);

        // Register host functions in the "construct" namespace
        Self::register_host_functions(&mut linker)?;

        let instance = linker
            .instantiate(&mut store, &module)
            .context("Failed to instantiate WASM module")?;

        // Grab the module's memory export
        if let Some(Extern::Memory(mem)) = instance.get_export(&mut store, "memory") {
            store.data_mut().memory = Some(mem);
        }

        // Call space_create
        let create_fn = instance
            .get_typed_func::<(), i32>(&mut store, "space_create")
            .context("Missing space_create export")?;
        let handle = create_fn
            .call(&mut store, ())
            .context("space_create failed")?;

        // Get space name
        let name = if let Ok(name_fn) =
            instance.get_typed_func::<i32, (i32, i32)>(&mut store, "space_name")
        {
            let (ptr, len) = name_fn
                .call(&mut store, handle)
                .unwrap_or((0, 0));
            if ptr != 0 && len > 0 {
                read_wasm_string(&store, ptr, len)
            } else {
                "WASM Space".to_string()
            }
        } else {
            "WASM Space".to_string()
        };

        Ok(Self {
            store,
            instance,
            handle,
            name,
        })
    }

    fn register_host_functions(linker: &mut Linker<WasmState>) -> Result<()> {
        // construct::fill_rect(x, y, w, h, r, g, b, a)
        linker.func_wrap(
            "construct",
            "fill_rect",
            |caller: Caller<'_, WasmState>,
             x: f64,
             y: f64,
             w: f64,
             h: f64,
             r: f64,
             g: f64,
             b: f64,
             a: f64| {
                let state = caller.data();
                let scene = state.scene;
                let xf = state.xf;
                if !scene.is_null() {
                    let color = Color::new([r as f32, g as f32, b as f32, a as f32]);
                    let rect = Rect::new(x, y, x + w, y + h);
                    unsafe { &mut *scene }.fill(Fill::NonZero, xf, color, None, &rect);
                }
            },
        )?;

        // construct::stroke_rect(x, y, w, h, r, g, b, a, line_width)
        linker.func_wrap(
            "construct",
            "stroke_rect",
            |caller: Caller<'_, WasmState>,
             x: f64,
             y: f64,
             w: f64,
             h: f64,
             r: f64,
             g: f64,
             b: f64,
             a: f64,
             line_width: f64| {
                let state = caller.data();
                let scene = state.scene;
                let xf = state.xf;
                if !scene.is_null() {
                    let color = Color::new([r as f32, g as f32, b as f32, a as f32]);
                    let rect = Rect::new(x, y, x + w, y + h);
                    unsafe { &mut *scene }.stroke(
                        &Stroke::new(line_width),
                        xf,
                        color,
                        None,
                        &rect,
                    );
                }
            },
        )?;

        // construct::fill_rounded_rect(x, y, w, h, radius, r, g, b, a)
        linker.func_wrap(
            "construct",
            "fill_rounded_rect",
            |caller: Caller<'_, WasmState>,
             x: f64,
             y: f64,
             w: f64,
             h: f64,
             radius: f64,
             r: f64,
             g: f64,
             b: f64,
             a: f64| {
                let state = caller.data();
                let scene = state.scene;
                let xf = state.xf;
                if !scene.is_null() {
                    let color = Color::new([r as f32, g as f32, b as f32, a as f32]);
                    let rrect =
                        RoundedRect::from_rect(Rect::new(x, y, x + w, y + h), radius);
                    unsafe { &mut *scene }.fill(Fill::NonZero, xf, color, None, &rrect);
                }
            },
        )?;

        // construct::fill_circle(cx, cy, radius, r, g, b, a)
        linker.func_wrap(
            "construct",
            "fill_circle",
            |caller: Caller<'_, WasmState>,
             cx: f64,
             cy: f64,
             radius: f64,
             r: f64,
             g: f64,
             b: f64,
             a: f64| {
                let state = caller.data();
                let scene = state.scene;
                let xf = state.xf;
                if !scene.is_null() {
                    let color = Color::new([r as f32, g as f32, b as f32, a as f32]);
                    let circle = Circle::new((cx, cy), radius);
                    unsafe { &mut *scene }.fill(Fill::NonZero, xf, color, None, &circle);
                }
            },
        )?;

        // construct::stroke_line(x1, y1, x2, y2, r, g, b, a, width)
        linker.func_wrap(
            "construct",
            "stroke_line",
            |caller: Caller<'_, WasmState>,
             x1: f64,
             y1: f64,
             x2: f64,
             y2: f64,
             r: f64,
             g: f64,
             b: f64,
             a: f64,
             width: f64| {
                let state = caller.data();
                let scene = state.scene;
                let xf = state.xf;
                if !scene.is_null() {
                    let color = Color::new([r as f32, g as f32, b as f32, a as f32]);
                    let line = Line::new((x1, y1), (x2, y2));
                    unsafe { &mut *scene }.stroke(
                        &Stroke::new(width),
                        xf,
                        color,
                        None,
                        &line,
                    );
                }
            },
        )?;

        // construct::draw_text(text_ptr, text_len, x, y, size, r, g, b, a)
        linker.func_wrap(
            "construct",
            "draw_text",
            |caller: Caller<'_, WasmState>,
             text_ptr: i32,
             text_len: i32,
             x: f64,
             y: f64,
             size: f64,
             r: f64,
             g: f64,
             b: f64,
             a: f64| {
                let state = caller.data();
                let scene = state.scene;
                let xf = state.xf;
                let font = state.font;
                if !scene.is_null() && !font.is_null() {
                    // Read text from WASM memory
                    let text = if let Some(memory) = state.memory {
                        let data = memory.data(&caller);
                        let start = text_ptr as usize;
                        let end = start + text_len as usize;
                        if end <= data.len() {
                            String::from_utf8_lossy(&data[start..end]).into_owned()
                        } else {
                            return;
                        }
                    } else {
                        return;
                    };
                    let color = Color::new([r as f32, g as f32, b as f32, a as f32]);
                    unsafe {
                        draw_text(&mut *scene, &*font, &text, x, y, size, color, xf);
                    }
                }
            },
        )?;

        // construct::clip_rect and clip_reset are stubs for now
        linker.func_wrap(
            "construct",
            "clip_rect",
            |_caller: Caller<'_, WasmState>,
             _x: f64,
             _y: f64,
             _w: f64,
             _h: f64| {
                // Clipping not yet supported in vello Scene directly;
                // would need push_layer. Stub for now.
            },
        )?;

        linker.func_wrap(
            "construct",
            "clip_reset",
            |_caller: Caller<'_, WasmState>| {
                // Stub
            },
        )?;

        Ok(())
    }
}

fn mouse_button_to_u32(button: MouseButton) -> u32 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
        MouseButton::Back => 3,
        MouseButton::Forward => 4,
        MouseButton::Other(n) => n as u32,
    }
}

fn modifiers_to_u32(modifiers: ModifiersState) -> u32 {
    let mut flags = 0u32;
    if modifiers.shift_key() {
        flags |= 1;
    }
    if modifiers.control_key() {
        flags |= 2;
    }
    if modifiers.alt_key() {
        flags |= 4;
    }
    if modifiers.super_key() {
        flags |= 8;
    }
    flags
}

impl Space for WasmSpace {
    fn name(&self) -> &str {
        &self.name
    }

    fn draw(
        &mut self,
        scene: &mut Scene,
        xf: Affine,
        area: Rect,
        font: &FontData,
        mono_font: &FontData,
    ) {
        // Set scene/font pointers so host functions can use them
        self.store.data_mut().scene = scene as *mut Scene;
        self.store.data_mut().xf = xf;
        self.store.data_mut().font = font as *const FontData;
        self.store.data_mut().mono_font = mono_font as *const FontData;

        if let Ok(draw_fn) = self.instance.get_typed_func::<(i32, f64, f64, f64, f64), ()>(
            &mut self.store,
            "space_draw",
        ) {
            let _ = draw_fn.call(
                &mut self.store,
                (self.handle, area.x0, area.y0, area.width(), area.height()),
            );
        }

        // Clear pointers after draw
        self.store.data_mut().scene = std::ptr::null_mut();
        self.store.data_mut().font = std::ptr::null();
        self.store.data_mut().mono_font = std::ptr::null();
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if let Ok(f) = self.instance.get_typed_func::<(i32, f64, f64, u32, f64, f64, f64, f64), ()>(
            &mut self.store,
            "space_mouse_click",
        ) {
            let _ = f.call(
                &mut self.store,
                (
                    self.handle,
                    x,
                    y,
                    mouse_button_to_u32(button),
                    area.x0,
                    area.y0,
                    area.width(),
                    area.height(),
                ),
            );
        }
    }

    fn handle_mouse_release(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if let Ok(f) = self.instance.get_typed_func::<(i32, f64, f64, u32, f64, f64, f64, f64), ()>(
            &mut self.store,
            "space_mouse_release",
        ) {
            let _ = f.call(
                &mut self.store,
                (
                    self.handle,
                    x,
                    y,
                    mouse_button_to_u32(button),
                    area.x0,
                    area.y0,
                    area.width(),
                    area.height(),
                ),
            );
        }
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        if let Ok(f) = self.instance.get_typed_func::<(i32, f64, f64, f64, f64, f64, f64), i32>(
            &mut self.store,
            "space_mouse_move",
        ) {
            f.call(
                &mut self.store,
                (
                    self.handle,
                    x,
                    y,
                    area.x0,
                    area.y0,
                    area.width(),
                    area.height(),
                ),
            )
            .unwrap_or(0)
                != 0
        } else {
            false
        }
    }

    fn handle_scroll(&mut self, dx: f64, dy: f64, mouse_x: f64, mouse_y: f64, area: Rect) {
        if let Ok(f) = self.instance.get_typed_func::<(i32, f64, f64, f64, f64, f64, f64, f64, f64), ()>(
            &mut self.store,
            "space_scroll",
        ) {
            let _ = f.call(
                &mut self.store,
                (
                    self.handle,
                    dx,
                    dy,
                    mouse_x,
                    mouse_y,
                    area.x0,
                    area.y0,
                    area.width(),
                    area.height(),
                ),
            );
        }
    }

    fn handle_key(&mut self, key: &Key, modifiers: ModifiersState) -> bool {
        if let Ok(f) = self
            .instance
            .get_typed_func::<(i32, u32, u32), i32>(&mut self.store, "space_key")
        {
            let key_code = match key {
                Key::Named(named) => *named as u32,
                Key::Character(ch) => ch.chars().next().unwrap_or('\0') as u32,
                _ => return false,
            };
            f.call(
                &mut self.store,
                (self.handle, key_code, modifiers_to_u32(modifiers)),
            )
            .unwrap_or(0)
                != 0
        } else {
            false
        }
    }

    fn handle_char(&mut self, ch: &str, modifiers: ModifiersState) -> bool {
        if let Ok(f) = self.instance.get_typed_func::<(i32, i32, i32, u32), i32>(
            &mut self.store,
            "space_char",
        ) {
            // We need to write the string into WASM memory
            let bytes = ch.as_bytes();

            // Try to allocate in WASM memory via an allocator export
            if let Ok(alloc) = self
                .instance
                .get_typed_func::<i32, i32>(&mut self.store, "alloc")
            {
                if let Ok(ptr) = alloc.call(&mut self.store, bytes.len() as i32) {
                    if let Some(memory) = self.store.data().memory {
                        let data = memory.data_mut(&mut self.store);
                        let start = ptr as usize;
                        let end = start + bytes.len();
                        if end <= data.len() {
                            data[start..end].copy_from_slice(bytes);
                            return f
                                .call(
                                    &mut self.store,
                                    (
                                        self.handle,
                                        ptr,
                                        bytes.len() as i32,
                                        modifiers_to_u32(modifiers),
                                    ),
                                )
                                .unwrap_or(0)
                                != 0;
                        }
                    }
                }
            }
        }
        false
    }
}
