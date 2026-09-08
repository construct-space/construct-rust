#![allow(dead_code)]

use vello::kurbo::{Affine, Circle, Line, Rect, Stroke};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 48.0;
const SIDEBAR_W: f64 = 200.0;
const SECTION_H: f64 = 36.0;
const FILE_ROW_H: f64 = 28.0;
const COMMIT_ROW_H: f64 = 52.0;
const FONT_SIZE: f64 = 13.0;
const SMALL_SIZE: f64 = 11.0;
const PAD: f64 = 12.0;

const GIT_BG: Color = Color::new([0.10, 0.10, 0.13, 1.0]);
const GIT_SIDEBAR: Color = Color::new([0.09, 0.09, 0.11, 1.0]);
const STAGED_COLOR: Color = Color::new([0.35, 0.80, 0.45, 1.0]);
const MODIFIED_COLOR: Color = Color::new([0.85, 0.65, 0.25, 1.0]);
const DELETED_COLOR: Color = Color::new([0.85, 0.35, 0.30, 1.0]);
const UNTRACKED_COLOR: Color = Color::new([0.55, 0.55, 0.60, 1.0]);
const BRANCH_COLOR: Color = Color::new([0.50, 0.75, 0.95, 1.0]);
const HASH_COLOR: Color = Color::new([0.65, 0.55, 0.85, 1.0]);
const GRAPH_COLOR: Color = Color::new([0.40, 0.70, 0.95, 1.0]);
const SECTION_BG: Color = Color::new([0.12, 0.12, 0.15, 1.0]);

#[derive(Clone, Copy)]
enum FileStatus {
    Staged,
    Modified,
    Deleted,
    Untracked,
}

impl FileStatus {
    fn color(self) -> Color {
        match self {
            FileStatus::Staged => STAGED_COLOR,
            FileStatus::Modified => MODIFIED_COLOR,
            FileStatus::Deleted => DELETED_COLOR,
            FileStatus::Untracked => UNTRACKED_COLOR,
        }
    }
    fn label(self) -> &'static str {
        match self {
            FileStatus::Staged => "S",
            FileStatus::Modified => "M",
            FileStatus::Deleted => "D",
            FileStatus::Untracked => "?",
        }
    }
}

struct GitFile {
    name: String,
    status: FileStatus,
}

struct GitCommit {
    hash: String,
    message: String,
    author: String,
    time: String,
}

pub struct GitSpace {
    branch: String,
    staged: Vec<GitFile>,
    unstaged: Vec<GitFile>,
    log: Vec<GitCommit>,
    scroll_y: f64,
    hovered_file: Option<(bool, usize)>, // (is_staged, index)
}

impl GitSpace {
    pub fn new() -> Self {
        Self {
            branch: "main".into(),
            staged: vec![
                GitFile { name: "src/plugin/mod.rs".into(), status: FileStatus::Staged },
                GitFile { name: "src/plugin/native.rs".into(), status: FileStatus::Staged },
            ],
            unstaged: vec![
                GitFile { name: "src/main.rs".into(), status: FileStatus::Modified },
                GitFile { name: "src/spaces/mod.rs".into(), status: FileStatus::Modified },
                GitFile { name: "src/spaces/notes.rs".into(), status: FileStatus::Untracked },
                GitFile { name: "Cargo.lock".into(), status: FileStatus::Modified },
            ],
            log: vec![
                GitCommit { hash: "a3f2b1c".into(), message: "feat: add plugin system with native + WASM support".into(), author: "dev".into(), time: "2 hours ago".into() },
                GitCommit { hash: "8e1d4f7".into(), message: "feat: implement Design and Code spaces".into(), author: "dev".into(), time: "5 hours ago".into() },
                GitCommit { hash: "c7b3a2e".into(), message: "init: vello + winit scaffold".into(), author: "dev".into(), time: "1 day ago".into() },
                GitCommit { hash: "1a0f9d3".into(), message: "chore: initial commit".into(), author: "dev".into(), time: "2 days ago".into() },
            ],
            scroll_y: 0.0,
            hovered_file: None,
        }
    }
}

impl Space for GitSpace {
    fn name(&self) -> &str {
        "Git"
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
        scene.fill(Fill::NonZero, xf, GIT_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Header
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + ww, ay + HEADER_H));
        draw_text(scene, font, "Git", ax + PAD, ay + HEADER_H * 0.5, FONT_SIZE + 2.0, TEXT_COLOR, xf);

        // Branch name
        let branch_label = format!("  {}", self.branch);
        draw_text(scene, font, &branch_label, ax + 50.0, ay + HEADER_H * 0.5, FONT_SIZE, BRANCH_COLOR, xf);

        // Staged count / unstaged count
        let status = format!("{}S  {}U", self.staged.len(), self.unstaged.len());
        draw_text(scene, font, &status, ax + ww - 80.0, ay + HEADER_H * 0.5, SMALL_SIZE, TEXT_SECONDARY, xf);

        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax, ay + HEADER_H - 1.0, ax + ww, ay + HEADER_H));

        // Changes panel (left half)
        let panel_w = ww * 0.45;
        let mut y = ay + HEADER_H - self.scroll_y;

        // Staged section
        scene.fill(Fill::NonZero, xf, SECTION_BG, None, &Rect::new(ax, y, ax + panel_w, y + SECTION_H));
        draw_text(scene, font, &format!("Staged Changes ({})", self.staged.len()), ax + PAD, y + SECTION_H * 0.7, FONT_SIZE, STAGED_COLOR, xf);
        y += SECTION_H;

        for (i, file) in self.staged.iter().enumerate() {
            if y + FILE_ROW_H >= ay + HEADER_H && y < ay + wh {
                if self.hovered_file == Some((true, i)) {
                    scene.fill(Fill::NonZero, xf, SIDEBAR_HOVER, None, &Rect::new(ax, y, ax + panel_w, y + FILE_ROW_H));
                }
                draw_text(scene, mono_font, file.status.label(), ax + PAD, y + FILE_ROW_H * 0.72, SMALL_SIZE, file.status.color(), xf);
                draw_text(scene, mono_font, &file.name, ax + PAD + 20.0, y + FILE_ROW_H * 0.72, SMALL_SIZE, TEXT_COLOR, xf);
            }
            y += FILE_ROW_H;
        }

        y += 4.0;

        // Unstaged section
        scene.fill(Fill::NonZero, xf, SECTION_BG, None, &Rect::new(ax, y, ax + panel_w, y + SECTION_H));
        draw_text(scene, font, &format!("Changes ({})", self.unstaged.len()), ax + PAD, y + SECTION_H * 0.7, FONT_SIZE, MODIFIED_COLOR, xf);
        y += SECTION_H;

        for (i, file) in self.unstaged.iter().enumerate() {
            if y + FILE_ROW_H >= ay + HEADER_H && y < ay + wh {
                if self.hovered_file == Some((false, i)) {
                    scene.fill(Fill::NonZero, xf, SIDEBAR_HOVER, None, &Rect::new(ax, y, ax + panel_w, y + FILE_ROW_H));
                }
                draw_text(scene, mono_font, file.status.label(), ax + PAD, y + FILE_ROW_H * 0.72, SMALL_SIZE, file.status.color(), xf);
                draw_text(scene, mono_font, &file.name, ax + PAD + 20.0, y + FILE_ROW_H * 0.72, SMALL_SIZE, TEXT_COLOR, xf);
            }
            y += FILE_ROW_H;
        }

        // Separator
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + panel_w, ay + HEADER_H, ax + panel_w + 1.0, ay + wh));

        // Commit log (right half)
        let log_x = ax + panel_w + 1.0;
        let _log_w = ww - panel_w - 1.0;
        let mut ly = ay + HEADER_H;

        scene.fill(Fill::NonZero, xf, SECTION_BG, None, &Rect::new(log_x, ly, ax + ww, ly + SECTION_H));
        draw_text(scene, font, "Commit Log", log_x + PAD, ly + SECTION_H * 0.7, FONT_SIZE, TEXT_COLOR, xf);
        ly += SECTION_H;

        for (i, commit) in self.log.iter().enumerate() {
            if ly + COMMIT_ROW_H >= ay + HEADER_H && ly < ay + wh {
                // Graph line
                let graph_x = log_x + 16.0;
                scene.fill(Fill::NonZero, xf, GRAPH_COLOR, None, &Circle::new((graph_x, ly + 14.0), 4.0));
                if i + 1 < self.log.len() {
                    scene.stroke(&Stroke::new(1.5), xf, GRAPH_COLOR, None, &Line::new((graph_x, ly + 18.0), (graph_x, ly + COMMIT_ROW_H)));
                }

                // Hash
                draw_text(scene, mono_font, &commit.hash, log_x + 32.0, ly + 18.0, SMALL_SIZE, HASH_COLOR, xf);

                // Message
                let msg = if commit.message.len() > 45 { format!("{}...", &commit.message[..42]) } else { commit.message.clone() };
                draw_text(scene, font, &msg, log_x + 100.0, ly + 18.0, FONT_SIZE, TEXT_COLOR, xf);

                // Author + time
                let meta = format!("{}  {}", commit.author, commit.time);
                draw_text(scene, font, &meta, log_x + 100.0, ly + 38.0, SMALL_SIZE, TEXT_SECONDARY, xf);
            }
            ly += COMMIT_ROW_H;
        }
    }

    fn handle_mouse_click(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        let panel_w = area.width() * 0.45;

        if mx < panel_w && my > HEADER_H {
            let mut cy = HEADER_H + SECTION_H;
            // Check staged files
            for i in 0..self.staged.len() {
                if my >= cy && my < cy + FILE_ROW_H {
                    let new = Some((true, i));
                    if new != self.hovered_file { self.hovered_file = new; return true; }
                    return false;
                }
                cy += FILE_ROW_H;
            }
            cy += 4.0 + SECTION_H;
            // Check unstaged files
            for i in 0..self.unstaged.len() {
                if my >= cy && my < cy + FILE_ROW_H {
                    let new = Some((false, i));
                    if new != self.hovered_file { self.hovered_file = new; return true; }
                    return false;
                }
                cy += FILE_ROW_H;
            }
        }
        if self.hovered_file.is_some() {
            self.hovered_file = None;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, _mouse_x: f64, _mouse_y: f64, _area: Rect) {
        self.scroll_y = (self.scroll_y - dy).max(0.0);
    }

    fn handle_key(&mut self, _key: &Key, _modifiers: ModifiersState) -> bool {
        false
    }

    fn handle_char(&mut self, _ch: &str, _modifiers: ModifiersState) -> bool {
        false
    }
}
