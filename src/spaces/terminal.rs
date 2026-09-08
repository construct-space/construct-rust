#![allow(dead_code)]

use std::time::Instant;

use vello::kurbo::{Affine, Rect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::draw_text;

use super::Space;

const LINE_H: f64 = 18.0;
const FONT_SIZE: f64 = 13.0;
const PAD: f64 = 8.0;

const TERM_BG: Color = Color::new([0.05, 0.05, 0.07, 1.0]);
const PROMPT_COLOR: Color = Color::new([0.35, 0.85, 0.45, 1.0]);
const OUTPUT_COLOR: Color = Color::new([0.78, 0.78, 0.80, 1.0]);
const ERROR_COLOR: Color = Color::new([0.9, 0.35, 0.35, 1.0]);
const DIR_DISPLAY: Color = Color::new([0.45, 0.65, 0.95, 1.0]);
const CURSOR_CLR: Color = Color::new([0.85, 0.85, 0.9, 1.0]);

#[derive(Clone)]
enum TermLine {
    Prompt(String),
    Output(String),
    Error(String),
}

pub struct TerminalSpace {
    lines: Vec<TermLine>,
    input: String,
    cursor_col: usize,
    cursor_visible: bool,
    cursor_blink: Instant,
    scroll_y: f64,
    cwd: String,
    char_width: f64,
}

impl TerminalSpace {
    pub fn new(mono_font: Option<&FontData>) -> Self {
        let cwd = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~".to_string());

        let char_width = mono_font
            .map(|f| crate::text::measure_char_width(f, FONT_SIZE))
            .unwrap_or(FONT_SIZE * 0.6);

        let mut lines = Vec::new();
        lines.push(TermLine::Output("Construct Terminal v0.1.0".to_string()));
        lines.push(TermLine::Output(String::new()));

        Self {
            lines,
            input: String::new(),
            cursor_col: 0,
            cursor_visible: true,
            cursor_blink: Instant::now(),
            scroll_y: 0.0,
            cwd,
            char_width,
        }
    }

    fn execute_command(&mut self) {
        let cmd = self.input.trim().to_string();
        let prompt = format!("{} $ {}", self.short_cwd(), cmd);
        self.lines.push(TermLine::Prompt(prompt));

        if cmd.is_empty() {
            self.input.clear();
            self.cursor_col = 0;
            return;
        }

        // Built-in commands
        if cmd == "clear" {
            self.lines.clear();
        } else if cmd == "pwd" {
            self.lines.push(TermLine::Output(self.cwd.clone()));
        } else if let Some(dir) = cmd.strip_prefix("cd ") {
            let dir = dir.trim();
            let path = if dir == "~" {
                dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/"))
            } else if dir.starts_with('/') {
                std::path::PathBuf::from(dir)
            } else {
                std::path::PathBuf::from(&self.cwd).join(dir)
            };
            match std::fs::canonicalize(&path) {
                Ok(p) if p.is_dir() => {
                    self.cwd = p.display().to_string();
                }
                _ => {
                    self.lines.push(TermLine::Error(format!("cd: no such directory: {}", dir)));
                }
            }
        } else if cmd == "help" {
            self.lines.push(TermLine::Output("Built-in: clear, pwd, cd, help, echo, ls".to_string()));
            self.lines.push(TermLine::Output("External commands also supported.".to_string()));
        } else if let Some(text) = cmd.strip_prefix("echo ") {
            self.lines.push(TermLine::Output(text.to_string()));
        } else if cmd == "ls" {
            match std::fs::read_dir(&self.cwd) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .flatten()
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            if e.path().is_dir() { format!("{}/", name) } else { name }
                        })
                        .collect();
                    names.sort();
                    for name in names {
                        self.lines.push(TermLine::Output(name));
                    }
                }
                Err(e) => {
                    self.lines.push(TermLine::Error(format!("ls: {}", e)));
                }
            }
        } else {
            // Try to run as shell command
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            if let Some((program, args)) = parts.split_first() {
                match std::process::Command::new(program)
                    .args(args)
                    .current_dir(&self.cwd)
                    .output()
                {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        for line in stdout.lines() {
                            self.lines.push(TermLine::Output(line.to_string()));
                        }
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        for line in stderr.lines() {
                            self.lines.push(TermLine::Error(line.to_string()));
                        }
                    }
                    Err(e) => {
                        self.lines.push(TermLine::Error(format!("{}: {}", program, e)));
                    }
                }
            }
        }

        self.input.clear();
        self.cursor_col = 0;
    }

    fn short_cwd(&self) -> String {
        if let Some(home) = dirs::home_dir() {
            let home_str = home.display().to_string();
            if self.cwd.starts_with(&home_str) {
                return format!("~{}", &self.cwd[home_str.len()..]);
            }
        }
        self.cwd.clone()
    }

    fn ensure_scroll_bottom(&mut self, area_h: f64) {
        let total = (self.lines.len() + 1) as f64 * LINE_H + PAD * 2.0;
        if total > area_h {
            self.scroll_y = total - area_h;
        }
    }
}

impl Space for TerminalSpace {
    fn name(&self) -> &str {
        "Terminal"
    }

    fn draw(
        &mut self,
        scene: &mut Scene,
        xf: Affine,
        area: Rect,
        _font: &FontData,
        mono_font: &FontData,
    ) {
        let ax = area.x0;
        let ay = area.y0;
        let ww = area.width();
        let wh = area.height();

        // Background
        scene.fill(Fill::NonZero, xf, TERM_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        let elapsed = self.cursor_blink.elapsed().as_secs_f64();
        self.cursor_visible = (elapsed * 2.0) as u64 % 2 == 0;

        // Draw history
        let mut y = ay + PAD - self.scroll_y;
        for line in &self.lines {
            if y + LINE_H >= ay && y < ay + wh {
                match line {
                    TermLine::Prompt(s) => draw_text(scene, mono_font, s, ax + PAD, y + LINE_H * 0.76, FONT_SIZE, PROMPT_COLOR, xf),
                    TermLine::Output(s) => {
                        if !s.is_empty() {
                            draw_text(scene, mono_font, s, ax + PAD, y + LINE_H * 0.76, FONT_SIZE, OUTPUT_COLOR, xf);
                        }
                    }
                    TermLine::Error(s) => draw_text(scene, mono_font, s, ax + PAD, y + LINE_H * 0.76, FONT_SIZE, ERROR_COLOR, xf),
                }
            }
            y += LINE_H;
        }

        // Current prompt line
        if y + LINE_H >= ay && y < ay + wh {
            let prompt = format!("{} $ ", self.short_cwd());
            draw_text(scene, mono_font, &prompt, ax + PAD, y + LINE_H * 0.76, FONT_SIZE, PROMPT_COLOR, xf);
            let prompt_w = prompt.chars().count() as f64 * self.char_width;
            if !self.input.is_empty() {
                draw_text(scene, mono_font, &self.input, ax + PAD + prompt_w, y + LINE_H * 0.76, FONT_SIZE, OUTPUT_COLOR, xf);
            }

            // Cursor
            if self.cursor_visible {
                let cx = ax + PAD + prompt_w + self.cursor_col as f64 * self.char_width;
                scene.fill(Fill::NonZero, xf, CURSOR_CLR, None, &Rect::new(cx, y + 2.0, cx + self.char_width, y + LINE_H - 2.0));
            }
        }
    }

    fn handle_mouse_click(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, _x: f64, _y: f64, _area: Rect) -> bool {
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, _mouse_x: f64, _mouse_y: f64, _area: Rect) {
        self.scroll_y = (self.scroll_y - dy).max(0.0);
    }

    fn handle_key(&mut self, key: &Key, _modifiers: ModifiersState) -> bool {
        match key {
            Key::Named(NamedKey::Enter) => {
                self.execute_command();
                self.ensure_scroll_bottom(800.0);
            }
            Key::Named(NamedKey::Backspace) => {
                if self.cursor_col > 0 {
                    let byte_idx = self.input.char_indices().nth(self.cursor_col).map(|(i, _)| i).unwrap_or(self.input.len());
                    let prev_byte = self.input.char_indices().nth(self.cursor_col - 1).map(|(i, _)| i).unwrap_or(0);
                    self.input.replace_range(prev_byte..byte_idx, "");
                    self.cursor_col -= 1;
                }
                self.cursor_visible = true;
                self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                if self.cursor_col > 0 { self.cursor_col -= 1; }
                self.cursor_visible = true;
                self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::ArrowRight) => {
                if self.cursor_col < self.input.chars().count() { self.cursor_col += 1; }
                self.cursor_visible = true;
                self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::Home) => {
                self.cursor_col = 0;
                self.cursor_visible = true;
                self.cursor_blink = Instant::now();
            }
            Key::Named(NamedKey::End) => {
                self.cursor_col = self.input.chars().count();
                self.cursor_visible = true;
                self.cursor_blink = Instant::now();
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
