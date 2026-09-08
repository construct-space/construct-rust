#![allow(dead_code)]

use std::time::Instant;

use vello::kurbo::{Affine, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 48.0;
const INPUT_H: f64 = 52.0;
const MSG_PAD: f64 = 12.0;
const BUBBLE_PAD: f64 = 10.0;
const BUBBLE_MAX_W: f64 = 400.0;
const FONT_SIZE: f64 = 13.0;
const SMALL_SIZE: f64 = 10.0;
const LINE_H: f64 = 18.0;

const CHAT_BG: Color = Color::new([0.10, 0.10, 0.13, 1.0]);
const INPUT_BG: Color = Color::new([0.13, 0.13, 0.16, 1.0]);
const MY_BUBBLE: Color = Color::new([0.22, 0.35, 0.65, 1.0]);
const THEIR_BUBBLE: Color = Color::new([0.18, 0.18, 0.22, 1.0]);
const MY_TEXT: Color = Color::new([0.95, 0.95, 0.98, 1.0]);
const THEIR_TEXT: Color = Color::new([0.85, 0.85, 0.88, 1.0]);
const TIME_COLOR: Color = Color::new([0.50, 0.50, 0.55, 1.0]);
const SEND_BTN: Color = Color::new([0.30, 0.55, 0.95, 1.0]);
const INPUT_FIELD_BG: Color = Color::new([0.16, 0.16, 0.20, 1.0]);

#[derive(Clone)]
struct Message {
    text: String,
    is_mine: bool,
    time: String,
}

pub struct ChatSpace {
    messages: Vec<Message>,
    input: String,
    cursor_col: usize,
    cursor_visible: bool,
    cursor_blink: Instant,
    scroll_y: f64,
}

impl ChatSpace {
    pub fn new() -> Self {
        Self {
            messages: vec![
                Message { text: "Hey! How's the project going?".into(), is_mine: false, time: "10:30".into() },
                Message { text: "Going well! Just finished the plugin system.".into(), is_mine: true, time: "10:32".into() },
                Message { text: "That's awesome. Native and WASM?".into(), is_mine: false, time: "10:33".into() },
                Message { text: "Yep, both. Native gets direct Scene access, WASM goes through host functions.".into(), is_mine: true, time: "10:34".into() },
                Message { text: "Nice architecture. What about error handling?".into(), is_mine: false, time: "10:35".into() },
                Message { text: "Plugin panics are caught — they render an error in the space area without crashing the host.".into(), is_mine: true, time: "10:36".into() },
            ],
            input: String::new(),
            cursor_col: 0,
            cursor_visible: true,
            cursor_blink: Instant::now(),
            scroll_y: 0.0,
        }
    }

    fn send_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.messages.push(Message {
            text,
            is_mine: true,
            time: "now".into(),
        });
        self.input.clear();
        self.cursor_col = 0;
    }

    fn msg_height(&self, msg: &Message) -> f64 {
        let chars_per_line = (BUBBLE_MAX_W / (FONT_SIZE * 0.55)) as usize;
        let lines = if chars_per_line > 0 {
            (msg.text.len() / chars_per_line) + 1
        } else {
            1
        };
        lines as f64 * LINE_H + BUBBLE_PAD * 2.0 + 16.0 // +16 for time
    }
}

impl Space for ChatSpace {
    fn name(&self) -> &str {
        "Chat"
    }

    fn draw(
        &mut self,
        scene: &mut Scene,
        xf: Affine,
        area: Rect,
        font: &FontData,
        mono_font: &FontData,
    ) {
        let ax = area.x0;
        let ay = area.y0;
        let ww = area.width();
        let wh = area.height();

        // Background
        scene.fill(Fill::NonZero, xf, CHAT_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Header
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + ww, ay + HEADER_H));
        draw_text(scene, font, "Chat", ax + 16.0, ay + HEADER_H * 0.65, FONT_SIZE + 2.0, TEXT_COLOR, xf);
        draw_text(scene, font, "online", ax + 70.0, ay + HEADER_H * 0.65, SMALL_SIZE, Color::new([0.35, 0.85, 0.45, 1.0]), xf);
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax, ay + HEADER_H - 1.0, ax + ww, ay + HEADER_H));

        // Messages area
        let msg_top = ay + HEADER_H;
        let msg_bottom = ay + wh - INPUT_H;
        let mut y = msg_top + MSG_PAD - self.scroll_y;

        for msg in &self.messages {
            let h = self.msg_height(msg);

            if y + h >= msg_top && y < msg_bottom {
                let bubble_w = (msg.text.len() as f64 * FONT_SIZE * 0.55 + BUBBLE_PAD * 2.0).min(BUBBLE_MAX_W);
                let bx = if msg.is_mine {
                    ax + ww - bubble_w - MSG_PAD
                } else {
                    ax + MSG_PAD
                };

                let bubble_rect = Rect::new(bx, y, bx + bubble_w, y + h - 16.0);
                let color = if msg.is_mine { MY_BUBBLE } else { THEIR_BUBBLE };
                scene.fill(Fill::NonZero, xf, color, None, &RoundedRect::from_rect(bubble_rect, 12.0));

                let text_color = if msg.is_mine { MY_TEXT } else { THEIR_TEXT };
                // Simple text wrapping
                let chars_per_line = ((bubble_w - BUBBLE_PAD * 2.0) / (FONT_SIZE * 0.55)) as usize;
                let chars_per_line = chars_per_line.max(1);
                let mut text_y = y + BUBBLE_PAD + LINE_H * 0.76;
                let mut remaining = msg.text.as_str();
                while !remaining.is_empty() {
                    let end = remaining.len().min(chars_per_line);
                    let chunk = &remaining[..end];
                    draw_text(scene, font, chunk, bx + BUBBLE_PAD, text_y, FONT_SIZE, text_color, xf);
                    remaining = &remaining[end..];
                    text_y += LINE_H;
                }

                // Time
                let time_x = if msg.is_mine { bx + bubble_w - 30.0 } else { bx };
                draw_text(scene, font, &msg.time, time_x, y + h - 6.0, SMALL_SIZE, TIME_COLOR, xf);
            }
            y += h + 4.0;
        }

        // Input area
        let input_y = ay + wh - INPUT_H;
        scene.fill(Fill::NonZero, xf, INPUT_BG, None, &Rect::new(ax, input_y, ax + ww, ay + wh));
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax, input_y, ax + ww, input_y + 1.0));

        // Input field
        let field_rect = Rect::new(ax + 12.0, input_y + 10.0, ax + ww - 70.0, input_y + INPUT_H - 10.0);
        scene.fill(Fill::NonZero, xf, INPUT_FIELD_BG, None, &RoundedRect::from_rect(field_rect, 16.0));

        let char_w = crate::text::measure_char_width(mono_font, FONT_SIZE);
        if !self.input.is_empty() {
            draw_text(scene, mono_font, &self.input, ax + 24.0, input_y + INPUT_H * 0.62, FONT_SIZE, TEXT_COLOR, xf);
        } else {
            draw_text(scene, font, "Type a message...", ax + 24.0, input_y + INPUT_H * 0.62, FONT_SIZE, TEXT_SECONDARY, xf);
        }

        // Cursor
        let elapsed = self.cursor_blink.elapsed().as_secs_f64();
        self.cursor_visible = (elapsed * 2.0) as u64 % 2 == 0;
        if self.cursor_visible {
            let cx = ax + 24.0 + self.cursor_col as f64 * char_w;
            let cy = input_y + 14.0;
            scene.fill(Fill::NonZero, xf, CURSOR_COLOR, None, &Rect::new(cx, cy, cx + 1.5, input_y + INPUT_H - 14.0));
        }

        // Send button
        let btn_rect = Rect::new(ax + ww - 58.0, input_y + 10.0, ax + ww - 12.0, input_y + INPUT_H - 10.0);
        scene.fill(Fill::NonZero, xf, SEND_BTN, None, &RoundedRect::from_rect(btn_rect, 16.0));
        draw_text(scene, font, "Send", ax + ww - 52.0, input_y + INPUT_H * 0.62, SMALL_SIZE + 1.0, Color::new([1.0, 1.0, 1.0, 1.0]), xf);
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left {
            return;
        }
        let mx = x - area.x0;
        let my = y - area.y0;
        let ww = area.width();
        let wh = area.height();

        // Send button
        if mx > ww - 58.0 && my > wh - INPUT_H + 10.0 {
            self.send_message();
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, _x: f64, _y: f64, _area: Rect) -> bool {
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, _mouse_x: f64, _mouse_y: f64, _area: Rect) {
        self.scroll_y = (self.scroll_y - dy).max(0.0);
    }

    fn handle_key(&mut self, key: &Key, _modifiers: ModifiersState) -> bool {
        match key {
            Key::Named(NamedKey::Enter) => { self.send_message(); }
            Key::Named(NamedKey::Backspace) => {
                if self.cursor_col > 0 {
                    let byte_idx = self.input.char_indices().nth(self.cursor_col).map(|(i, _)| i).unwrap_or(self.input.len());
                    let prev = self.input.char_indices().nth(self.cursor_col - 1).map(|(i, _)| i).unwrap_or(0);
                    self.input.replace_range(prev..byte_idx, "");
                    self.cursor_col -= 1;
                }
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if self.cursor_col > 0 { self.cursor_col -= 1; }
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowRight) => {
                if self.cursor_col < self.input.chars().count() { self.cursor_col += 1; }
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            _ => return false,
        }
        true
    }

    fn handle_char(&mut self, ch: &str, modifiers: ModifiersState) -> bool {
        if modifiers.super_key() || modifiers.control_key() { return false; }
        for c in ch.chars() {
            let byte_idx = self.input.char_indices().nth(self.cursor_col).map(|(i, _)| i).unwrap_or(self.input.len());
            self.input.insert(byte_idx, c);
            self.cursor_col += 1;
        }
        self.cursor_visible = true;
        self.cursor_blink = Instant::now();
        true
    }
}
