#![allow(dead_code)]

use std::time::Instant;

use vello::kurbo::{Affine, Circle, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 52.0;
const INPUT_H: f64 = 56.0;
const MSG_PAD: f64 = 16.0;
const LINE_H: f64 = 20.0;
const FONT_SIZE: f64 = 14.0;
const SMALL_SIZE: f64 = 11.0;
const SIDEBAR_W: f64 = 220.0;

const AI_BG: Color = Color::new([0.09, 0.09, 0.12, 1.0]);
const AI_SIDEBAR: Color = Color::new([0.08, 0.08, 0.10, 1.0]);
const AI_ACCENT: Color = Color::new([0.65, 0.45, 0.95, 1.0]);
const USER_BUBBLE: Color = Color::new([0.16, 0.16, 0.20, 1.0]);
const AI_BUBBLE: Color = Color::new([0.12, 0.14, 0.20, 1.0]);
const INPUT_FIELD: Color = Color::new([0.14, 0.14, 0.18, 1.0]);
const CONV_HOVER: Color = Color::new([0.12, 0.12, 0.16, 1.0]);
const CONV_ACTIVE: Color = Color::new([0.16, 0.18, 0.28, 1.0]);

#[derive(Clone)]
struct AiMessage {
    text: String,
    is_user: bool,
}

struct Conversation {
    title: String,
    messages: Vec<AiMessage>,
}

pub struct AiSpace {
    conversations: Vec<Conversation>,
    active_conv: usize,
    input: String,
    cursor_col: usize,
    cursor_visible: bool,
    cursor_blink: Instant,
    scroll_y: f64,
    hovered_conv: Option<usize>,
}

impl AiSpace {
    pub fn new() -> Self {
        Self {
            conversations: vec![
                Conversation {
                    title: "Architecture Review".into(),
                    messages: vec![
                        AiMessage { text: "Review my plugin system architecture".into(), is_user: true },
                        AiMessage { text: "Your plugin system looks well-designed. The separation between native (dlopen with C ABI) and WASM (wasmtime with host functions) gives you the best of both worlds: zero-cost rendering for trusted plugins and sandboxed execution for untrusted ones.".into(), is_user: false },
                        AiMessage { text: "What about error handling?".into(), is_user: true },
                        AiMessage { text: "I'd recommend wrapping each Space trait call in std::panic::catch_unwind for native plugins. For WASM, wasmtime already provides trap handling. Both should render a fallback error UI in the space area rather than crashing the host.".into(), is_user: false },
                    ],
                },
                Conversation {
                    title: "Code Generation".into(),
                    messages: vec![
                        AiMessage { text: "Generate a WASM space template".into(), is_user: true },
                        AiMessage { text: "Here's a minimal WASM space template in Rust targeting wasm32-unknown-unknown. It exports the required functions: space_create, space_name, space_draw, and uses the construct host functions for rendering.".into(), is_user: false },
                    ],
                },
            ],
            active_conv: 0,
            input: String::new(),
            cursor_col: 0,
            cursor_visible: true,
            cursor_blink: Instant::now(),
            scroll_y: 0.0,
            hovered_conv: None,
        }
    }

    fn send_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() { return; }
        self.conversations[self.active_conv].messages.push(AiMessage { text: text.clone(), is_user: true });
        // Simulated AI response
        self.conversations[self.active_conv].messages.push(AiMessage {
            text: "I'm thinking about your request... (AI responses would appear here in a real implementation)".into(),
            is_user: false,
        });
        self.input.clear();
        self.cursor_col = 0;
    }
}

impl Space for AiSpace {
    fn name(&self) -> &str {
        "AI"
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
        scene.fill(Fill::NonZero, xf, AI_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Sidebar
        scene.fill(Fill::NonZero, xf, AI_SIDEBAR, None, &Rect::new(ax, ay, ax + SIDEBAR_W, ay + wh));

        // Sidebar header
        scene.fill(Fill::NonZero, xf, AI_ACCENT, None, &Circle::new((ax + 24.0, ay + 28.0), 10.0));
        draw_text(scene, font, "AI", ax + 18.0, ay + 33.0, 11.0, Color::new([1.0, 1.0, 1.0, 1.0]), xf);
        draw_text(scene, font, "Conversations", ax + 44.0, ay + 33.0, FONT_SIZE, TEXT_COLOR, xf);
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax, ay + 52.0, ax + SIDEBAR_W, ay + 53.0));

        // Conversation list
        for (i, conv) in self.conversations.iter().enumerate() {
            let y = ay + 56.0 + i as f64 * 40.0;
            let bg = if i == self.active_conv {
                CONV_ACTIVE
            } else if self.hovered_conv == Some(i) {
                CONV_HOVER
            } else {
                AI_SIDEBAR
            };
            scene.fill(Fill::NonZero, xf, bg, None, &Rect::new(ax, y, ax + SIDEBAR_W, y + 40.0));
            let title = if conv.title.len() > 22 { format!("{}...", &conv.title[..19]) } else { conv.title.clone() };
            draw_text(scene, font, &title, ax + 16.0, y + 26.0, SMALL_SIZE + 1.0, TEXT_COLOR, xf);
        }

        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + SIDEBAR_W - 1.0, ay, ax + SIDEBAR_W, ay + wh));

        // Main chat area header
        let chat_x = ax + SIDEBAR_W;
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(chat_x, ay, ax + ww, ay + HEADER_H));
        let conv = &self.conversations[self.active_conv];
        draw_text(scene, font, &conv.title, chat_x + 16.0, ay + 32.0, FONT_SIZE + 1.0, TEXT_COLOR, xf);
        draw_text(scene, font, "Claude", chat_x + 16.0, ay + HEADER_H - 8.0, SMALL_SIZE, AI_ACCENT, xf);
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(chat_x, ay + HEADER_H - 1.0, ax + ww, ay + HEADER_H));

        // Messages
        let msg_area_w = ww - SIDEBAR_W;
        let mut y = ay + HEADER_H + MSG_PAD - self.scroll_y;
        let msg_bottom = ay + wh - INPUT_H;

        for msg in &conv.messages {
            let chars_per_line = ((msg_area_w - MSG_PAD * 4.0) / (FONT_SIZE * 0.52)) as usize;
            let chars_per_line = chars_per_line.max(1);
            let n_lines = (msg.text.len() / chars_per_line) + 1;
            let bubble_h = n_lines as f64 * LINE_H + 16.0;

            if y + bubble_h >= ay + HEADER_H && y < msg_bottom {
                let bubble_w = (msg.text.len() as f64 * FONT_SIZE * 0.52 + 24.0).min(msg_area_w - MSG_PAD * 2.0);
                let bx = if msg.is_user {
                    ax + ww - bubble_w - MSG_PAD
                } else {
                    chat_x + MSG_PAD
                };

                let bg = if msg.is_user { USER_BUBBLE } else { AI_BUBBLE };
                scene.fill(Fill::NonZero, xf, bg, None, &RoundedRect::from_rect(Rect::new(bx, y, bx + bubble_w, y + bubble_h), 10.0));

                if !msg.is_user {
                    scene.fill(Fill::NonZero, xf, AI_ACCENT, None, &Circle::new((bx - 14.0, y + 12.0), 6.0));
                }

                let text_color = TEXT_COLOR;
                let mut text_y = y + 8.0 + LINE_H * 0.76;
                let mut remaining = msg.text.as_str();
                while !remaining.is_empty() {
                    let end = remaining.len().min(chars_per_line);
                    let chunk = &remaining[..end];
                    draw_text(scene, font, chunk, bx + 12.0, text_y, FONT_SIZE, text_color, xf);
                    remaining = &remaining[end..];
                    text_y += LINE_H;
                }
            }
            y += bubble_h + 8.0;
        }

        // Input area
        let input_y = ay + wh - INPUT_H;
        scene.fill(Fill::NonZero, xf, AI_SIDEBAR, None, &Rect::new(chat_x, input_y, ax + ww, ay + wh));
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(chat_x, input_y, ax + ww, input_y + 1.0));

        let field = Rect::new(chat_x + 16.0, input_y + 12.0, ax + ww - 72.0, input_y + INPUT_H - 12.0);
        scene.fill(Fill::NonZero, xf, INPUT_FIELD, None, &RoundedRect::from_rect(field, 16.0));

        let char_w = crate::text::measure_char_width(mono_font, FONT_SIZE);
        if !self.input.is_empty() {
            draw_text(scene, mono_font, &self.input, chat_x + 28.0, input_y + INPUT_H * 0.58, FONT_SIZE, TEXT_COLOR, xf);
        } else {
            draw_text(scene, font, "Ask AI...", chat_x + 28.0, input_y + INPUT_H * 0.58, FONT_SIZE, TEXT_SECONDARY, xf);
        }

        // Cursor
        let elapsed = self.cursor_blink.elapsed().as_secs_f64();
        self.cursor_visible = (elapsed * 2.0) as u64 % 2 == 0;
        if self.cursor_visible {
            let cx = chat_x + 28.0 + self.cursor_col as f64 * char_w;
            scene.fill(Fill::NonZero, xf, CURSOR_COLOR, None, &Rect::new(cx, input_y + 16.0, cx + 1.5, input_y + INPUT_H - 16.0));
        }

        // Send button
        let btn = Rect::new(ax + ww - 60.0, input_y + 12.0, ax + ww - 16.0, input_y + INPUT_H - 12.0);
        scene.fill(Fill::NonZero, xf, AI_ACCENT, None, &RoundedRect::from_rect(btn, 16.0));
        draw_text(scene, font, ">", ax + ww - 43.0, input_y + INPUT_H * 0.58, FONT_SIZE + 2.0, Color::new([1.0, 1.0, 1.0, 1.0]), xf);
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left { return; }
        let mx = x - area.x0;
        let my = y - area.y0;
        let ww = area.width();
        let wh = area.height();

        if mx < SIDEBAR_W && my > 56.0 {
            let idx = ((my - 56.0) / 40.0) as usize;
            if idx < self.conversations.len() {
                self.active_conv = idx;
                self.scroll_y = 0.0;
            }
        } else if mx > ww - 60.0 && my > wh - INPUT_H + 12.0 {
            self.send_message();
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        if mx < SIDEBAR_W && my > 56.0 {
            let idx = ((my - 56.0) / 40.0) as usize;
            let new_hover = if idx < self.conversations.len() { Some(idx) } else { None };
            if new_hover != self.hovered_conv {
                self.hovered_conv = new_hover;
                return true;
            }
        } else if self.hovered_conv.is_some() {
            self.hovered_conv = None;
            return true;
        }
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
