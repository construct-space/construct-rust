#![allow(dead_code)]

use std::time::Instant;

use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const LIST_W: f64 = 240.0;
const NOTE_ROW_H: f64 = 64.0;
const HEADER_H: f64 = 44.0;
const TEXT_PAD: f64 = 12.0;
const FONT_SIZE: f64 = 14.0;
const TITLE_SIZE: f64 = 13.0;
const PREVIEW_SIZE: f64 = 11.0;
const EDITOR_FONT_SIZE: f64 = 15.0;
const LINE_H: f64 = 22.0;

const NOTES_BG: Color = Color::new([0.11, 0.11, 0.14, 1.0]);
const LIST_BG: Color = Color::new([0.10, 0.10, 0.13, 1.0]);
const NOTE_HOVER: Color = Color::new([0.15, 0.15, 0.19, 1.0]);
const NOTE_SELECTED: Color = Color::new([0.18, 0.22, 0.38, 1.0]);
const TITLE_COLOR: Color = Color::new([0.88, 0.88, 0.92, 1.0]);
const PREVIEW_COLOR: Color = Color::new([0.55, 0.55, 0.60, 1.0]);
const DATE_COLOR: Color = Color::new([0.45, 0.45, 0.50, 1.0]);
const ACCENT: Color = Color::new([0.95, 0.75, 0.30, 1.0]);

#[derive(Debug, Clone)]
struct Note {
    title: String,
    lines: Vec<String>,
}

impl Note {
    fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.to_string(),
            lines: content.lines().map(|l| l.to_string()).collect(),
        }
    }

    fn preview(&self) -> &str {
        self.lines.first().map(|s| s.as_str()).unwrap_or("")
    }

    fn full_text(&self) -> String {
        self.lines.join("\n")
    }
}

pub struct NotesSpace {
    notes: Vec<Note>,
    selected: usize,
    cursor_line: usize,
    cursor_col: usize,
    cursor_visible: bool,
    cursor_blink: Instant,
    scroll_y: f64,
    list_scroll_y: f64,
    hovered_note: Option<usize>,
}

impl NotesSpace {
    pub fn new() -> Self {
        let notes = vec![
            Note::new("Welcome to Notes", "This is your notes space.\nWrite down ideas, thoughts, and more.\n\nUse the sidebar to switch between notes."),
            Note::new("Todo List", "- Build plugin system\n- Implement all spaces\n- Test everything\n- Ship it"),
            Note::new("Meeting Notes", "Project sync - Feb 2026\n\nDiscussed architecture decisions.\nAgreed on vello for rendering.\nNext steps: implement spaces."),
        ];
        Self {
            notes,
            selected: 0,
            cursor_line: 0,
            cursor_col: 0,
            cursor_visible: true,
            cursor_blink: Instant::now(),
            scroll_y: 0.0,
            list_scroll_y: 0.0,
            hovered_note: None,
        }
    }

    fn insert_char(&mut self, ch: char) {
        let note = &mut self.notes[self.selected];
        if self.cursor_line >= note.lines.len() {
            note.lines.push(String::new());
        }
        let line = &note.lines[self.cursor_line];
        let byte_idx = line
            .char_indices()
            .nth(self.cursor_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        let mut new_line = line.clone();
        new_line.insert(byte_idx, ch);
        note.lines[self.cursor_line] = new_line;
        self.cursor_col += 1;
    }

    fn insert_newline(&mut self) {
        let note = &mut self.notes[self.selected];
        if self.cursor_line >= note.lines.len() {
            note.lines.push(String::new());
            self.cursor_line = note.lines.len() - 1;
            self.cursor_col = 0;
            return;
        }
        let line = note.lines[self.cursor_line].clone();
        let byte_idx = line
            .char_indices()
            .nth(self.cursor_col)
            .map(|(i, _)| i)
            .unwrap_or(line.len());
        let before = line[..byte_idx].to_string();
        let after = line[byte_idx..].to_string();
        note.lines[self.cursor_line] = before;
        note.lines.insert(self.cursor_line + 1, after);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    fn backspace(&mut self) {
        let note = &mut self.notes[self.selected];
        if self.cursor_col > 0 {
            let line = &note.lines[self.cursor_line];
            let byte_idx = line
                .char_indices()
                .nth(self.cursor_col)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            let prev_byte = line
                .char_indices()
                .nth(self.cursor_col - 1)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let mut new_line = line.clone();
            new_line.replace_range(prev_byte..byte_idx, "");
            note.lines[self.cursor_line] = new_line;
            self.cursor_col -= 1;
        } else if self.cursor_line > 0 {
            let current = note.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = note.lines[self.cursor_line].chars().count();
            note.lines[self.cursor_line].push_str(&current);
            self.cursor_col = prev_len;
        }
    }
}

impl Space for NotesSpace {
    fn name(&self) -> &str {
        "Notes"
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
        scene.fill(Fill::NonZero, xf, NOTES_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Note list panel
        scene.fill(Fill::NonZero, xf, LIST_BG, None, &Rect::new(ax, ay, ax + LIST_W, ay + wh));

        // Header
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + LIST_W, ay + HEADER_H));
        draw_text(scene, font, "Notes", ax + TEXT_PAD, ay + HEADER_H * 0.68, FONT_SIZE, ACCENT, xf);
        let count = format!("{}", self.notes.len());
        draw_text(scene, font, &count, ax + LIST_W - 30.0, ay + HEADER_H * 0.68, PREVIEW_SIZE, DATE_COLOR, xf);

        // Note list
        for (i, note) in self.notes.iter().enumerate() {
            let y = ay + HEADER_H + i as f64 * NOTE_ROW_H - self.list_scroll_y;
            if y + NOTE_ROW_H < ay + HEADER_H || y > ay + wh {
                continue;
            }

            let bg = if i == self.selected {
                NOTE_SELECTED
            } else if self.hovered_note == Some(i) {
                NOTE_HOVER
            } else {
                LIST_BG
            };
            scene.fill(Fill::NonZero, xf, bg, None, &Rect::new(ax, y, ax + LIST_W, y + NOTE_ROW_H));

            // Title
            let title = if note.title.len() > 28 {
                format!("{}...", &note.title[..25])
            } else {
                note.title.clone()
            };
            draw_text(scene, font, &title, ax + TEXT_PAD, y + 24.0, TITLE_SIZE, TITLE_COLOR, xf);

            // Preview
            let preview = note.preview();
            let preview = if preview.len() > 35 {
                format!("{}...", &preview[..32])
            } else {
                preview.to_string()
            };
            draw_text(scene, font, &preview, ax + TEXT_PAD, y + 46.0, PREVIEW_SIZE, PREVIEW_COLOR, xf);

            // Separator
            scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + TEXT_PAD, y + NOTE_ROW_H - 1.0, ax + LIST_W - TEXT_PAD, y + NOTE_ROW_H));
        }

        // List/editor separator
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + LIST_W - 1.0, ay, ax + LIST_W, ay + wh));

        // Editor area
        let editor_x = ax + LIST_W + TEXT_PAD;
        let editor_w = ww - LIST_W - TEXT_PAD * 2.0;

        // Editor header with note title
        let note = &self.notes[self.selected];
        draw_text(scene, font, &note.title, editor_x, ay + HEADER_H * 0.68, FONT_SIZE + 2.0, TITLE_COLOR, xf);
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + LIST_W, ay + HEADER_H - 1.0, ax + ww, ay + HEADER_H));

        // Editor content
        let editor_top = ay + HEADER_H + 8.0;
        let elapsed = self.cursor_blink.elapsed().as_secs_f64();
        self.cursor_visible = (elapsed * 2.0) as u64 % 2 == 0;

        let char_w = 8.4; // approximate
        for (i, line) in note.lines.iter().enumerate() {
            let y = editor_top + i as f64 * LINE_H - self.scroll_y;
            if y + LINE_H < editor_top || y > ay + wh {
                continue;
            }
            if !line.is_empty() {
                draw_text(scene, mono_font, line, editor_x, y + LINE_H * 0.76, EDITOR_FONT_SIZE, TEXT_COLOR, xf);
            }
        }

        // Cursor
        if self.cursor_visible {
            let cy = editor_top + self.cursor_line as f64 * LINE_H - self.scroll_y;
            let cx = editor_x + self.cursor_col as f64 * char_w;
            if cy >= editor_top && cy < ay + wh {
                scene.fill(Fill::NonZero, xf, CURSOR_COLOR, None, &Rect::new(cx, cy + 2.0, cx + 1.5, cy + LINE_H - 2.0));
            }
        }

        let _ = (editor_w, mono_font);
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left {
            return;
        }
        let mx = x - area.x0;
        let my = y - area.y0;

        if mx < LIST_W && my > HEADER_H {
            let idx = ((my - HEADER_H + self.list_scroll_y) / NOTE_ROW_H) as usize;
            if idx < self.notes.len() {
                self.selected = idx;
                self.cursor_line = 0;
                self.cursor_col = 0;
                self.scroll_y = 0.0;
            }
        } else if mx >= LIST_W {
            let editor_top = HEADER_H + 8.0;
            let ey = my - editor_top + self.scroll_y;
            let line = (ey / LINE_H) as usize;
            let ex = mx - LIST_W - TEXT_PAD;
            let col = if ex > 0.0 { (ex / 8.4).round() as usize } else { 0 };
            let note = &self.notes[self.selected];
            self.cursor_line = line.min(note.lines.len().saturating_sub(1));
            let line_len = note.lines.get(self.cursor_line).map(|l| l.chars().count()).unwrap_or(0);
            self.cursor_col = col.min(line_len);
            self.cursor_visible = true;
            self.cursor_blink = Instant::now();
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        if mx < LIST_W && my > HEADER_H {
            let idx = ((my - HEADER_H + self.list_scroll_y) / NOTE_ROW_H) as usize;
            let new_hover = if idx < self.notes.len() { Some(idx) } else { None };
            if new_hover != self.hovered_note {
                self.hovered_note = new_hover;
                return true;
            }
        } else if self.hovered_note.is_some() {
            self.hovered_note = None;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, mouse_x: f64, _mouse_y: f64, area: Rect) {
        let mx = mouse_x - area.x0;
        if mx < LIST_W {
            self.list_scroll_y = (self.list_scroll_y - dy).max(0.0);
        } else {
            self.scroll_y = (self.scroll_y - dy).max(0.0);
        }
    }

    fn handle_key(&mut self, key: &Key, _modifiers: ModifiersState) -> bool {
        match key {
            Key::Named(NamedKey::Backspace) => { self.backspace(); self.cursor_visible = true; self.cursor_blink = Instant::now(); }
            Key::Named(NamedKey::Enter) => { self.insert_newline(); self.cursor_visible = true; self.cursor_blink = Instant::now(); }
            Key::Named(NamedKey::ArrowUp) => {
                if self.cursor_line > 0 { self.cursor_line -= 1; }
                let note = &self.notes[self.selected];
                let len = note.lines.get(self.cursor_line).map(|l| l.chars().count()).unwrap_or(0);
                self.cursor_col = self.cursor_col.min(len);
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowDown) => {
                let note = &self.notes[self.selected];
                if self.cursor_line + 1 < note.lines.len() { self.cursor_line += 1; }
                let len = note.lines.get(self.cursor_line).map(|l| l.chars().count()).unwrap_or(0);
                self.cursor_col = self.cursor_col.min(len);
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if self.cursor_col > 0 { self.cursor_col -= 1; }
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowRight) => {
                let note = &self.notes[self.selected];
                let len = note.lines.get(self.cursor_line).map(|l| l.chars().count()).unwrap_or(0);
                if self.cursor_col < len { self.cursor_col += 1; }
                self.cursor_visible = true; self.cursor_blink = Instant::now();
            }
            _ => return false,
        }
        true
    }

    fn handle_char(&mut self, ch: &str, modifiers: ModifiersState) -> bool {
        if modifiers.super_key() || modifiers.control_key() { return false; }
        for c in ch.chars() {
            self.insert_char(c);
        }
        self.cursor_visible = true;
        self.cursor_blink = Instant::now();
        true
    }
}
