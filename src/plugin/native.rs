use std::ffi::CStr;
use std::os::raw::c_void;
use std::path::Path;

use anyhow::{Context, Result};
use vello::kurbo::{Affine, Rect};
use vello::peniko::FontData;
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::spaces::Space;

type CreateFn = unsafe extern "C" fn() -> *mut c_void;
type DestroyFn = unsafe extern "C" fn(*mut c_void);
type NameFn = unsafe extern "C" fn(*const c_void) -> *const i8;
type DrawFn = unsafe extern "C" fn(
    *mut c_void,
    *mut Scene,
    *const Affine,
    *const Rect,
    *const FontData,
    *const FontData,
);
type MouseClickFn = unsafe extern "C" fn(*mut c_void, f64, f64, u32, *const Rect);
type MouseReleaseFn = unsafe extern "C" fn(*mut c_void, f64, f64, u32, *const Rect);
type MouseMoveFn = unsafe extern "C" fn(*mut c_void, f64, f64, *const Rect) -> i32;
type ScrollFn = unsafe extern "C" fn(*mut c_void, f64, f64, f64, f64, *const Rect);
type KeyFn = unsafe extern "C" fn(*mut c_void, u32, u32) -> i32;
type CharFn = unsafe extern "C" fn(*mut c_void, *const u8, u32, u32) -> i32;

pub struct NativeSpace {
    _lib: libloading::Library,
    instance: *mut c_void,
    name: String,
    draw_fn: DrawFn,
    mouse_click_fn: Option<MouseClickFn>,
    mouse_release_fn: Option<MouseReleaseFn>,
    mouse_move_fn: Option<MouseMoveFn>,
    scroll_fn: Option<ScrollFn>,
    key_fn: Option<KeyFn>,
    char_fn: Option<CharFn>,
    destroy_fn: Option<DestroyFn>,
}

// NativeSpace holds a raw pointer but we manage its lifetime via Drop
unsafe impl Send for NativeSpace {}

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

impl NativeSpace {
    pub fn load(lib_path: &Path) -> Result<Self> {
        unsafe {
            let lib = libloading::Library::new(lib_path)
                .with_context(|| format!("Failed to load native plugin: {}", lib_path.display()))?;

            let create: libloading::Symbol<CreateFn> = lib
                .get(b"construct_space_create")
                .context("Missing construct_space_create symbol")?;
            let instance = create();
            if instance.is_null() {
                anyhow::bail!("construct_space_create returned null");
            }

            let name_fn: libloading::Symbol<NameFn> = lib
                .get(b"construct_space_name")
                .context("Missing construct_space_name symbol")?;
            let name_ptr = name_fn(instance);
            let name = if name_ptr.is_null() {
                "Unknown".to_string()
            } else {
                CStr::from_ptr(name_ptr).to_string_lossy().into_owned()
            };

            let draw_fn: DrawFn = *lib
                .get::<DrawFn>(b"construct_space_draw")
                .context("Missing construct_space_draw symbol")?;

            let mouse_click_fn = lib
                .get::<MouseClickFn>(b"construct_space_mouse_click")
                .ok()
                .map(|s| *s);
            let mouse_release_fn = lib
                .get::<MouseReleaseFn>(b"construct_space_mouse_release")
                .ok()
                .map(|s| *s);
            let mouse_move_fn = lib
                .get::<MouseMoveFn>(b"construct_space_mouse_move")
                .ok()
                .map(|s| *s);
            let scroll_fn = lib
                .get::<ScrollFn>(b"construct_space_scroll")
                .ok()
                .map(|s| *s);
            let key_fn = lib.get::<KeyFn>(b"construct_space_key").ok().map(|s| *s);
            let char_fn = lib
                .get::<CharFn>(b"construct_space_char")
                .ok()
                .map(|s| *s);
            let destroy_fn = lib
                .get::<DestroyFn>(b"construct_space_destroy")
                .ok()
                .map(|s| *s);

            Ok(Self {
                _lib: lib,
                instance,
                name,
                draw_fn,
                mouse_click_fn,
                mouse_release_fn,
                mouse_move_fn,
                scroll_fn,
                key_fn,
                char_fn,
                destroy_fn,
            })
        }
    }
}

impl Drop for NativeSpace {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy_fn {
            unsafe {
                destroy(self.instance);
            }
        }
    }
}

impl Space for NativeSpace {
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
        unsafe {
            (self.draw_fn)(
                self.instance,
                scene as *mut Scene,
                &xf as *const Affine,
                &area as *const Rect,
                font as *const FontData,
                mono_font as *const FontData,
            );
        }
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if let Some(f) = self.mouse_click_fn {
            unsafe {
                f(self.instance, x, y, mouse_button_to_u32(button), &area);
            }
        }
    }

    fn handle_mouse_release(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if let Some(f) = self.mouse_release_fn {
            unsafe {
                f(self.instance, x, y, mouse_button_to_u32(button), &area);
            }
        }
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        if let Some(f) = self.mouse_move_fn {
            unsafe { f(self.instance, x, y, &area) != 0 }
        } else {
            false
        }
    }

    fn handle_scroll(&mut self, dx: f64, dy: f64, mouse_x: f64, mouse_y: f64, area: Rect) {
        if let Some(f) = self.scroll_fn {
            unsafe {
                f(self.instance, dx, dy, mouse_x, mouse_y, &area);
            }
        }
    }

    fn handle_key(&mut self, key: &Key, modifiers: ModifiersState) -> bool {
        if let Some(f) = self.key_fn {
            let key_code = match key {
                Key::Named(named) => *named as u32,
                Key::Character(ch) => {
                    let first = ch.chars().next().unwrap_or('\0');
                    first as u32
                }
                _ => return false,
            };
            unsafe { f(self.instance, key_code, modifiers_to_u32(modifiers)) != 0 }
        } else {
            false
        }
    }

    fn handle_char(&mut self, ch: &str, modifiers: ModifiersState) -> bool {
        if let Some(f) = self.char_fn {
            let bytes = ch.as_bytes();
            unsafe {
                f(
                    self.instance,
                    bytes.as_ptr(),
                    bytes.len() as u32,
                    modifiers_to_u32(modifiers),
                ) != 0
            }
        } else {
            false
        }
    }
}
