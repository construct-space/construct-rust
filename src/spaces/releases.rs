#![allow(dead_code)]

use vello::kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 48.0;
const SIDEBAR_W: f64 = 260.0;
const RELEASE_ROW_H: f64 = 64.0;
const PAD: f64 = 16.0;
const FONT_SIZE: f64 = 13.0;
const SMALL_SIZE: f64 = 11.0;
const HEADING_SIZE: f64 = 18.0;

const REL_BG: Color = Color::new([0.10, 0.10, 0.13, 1.0]);
const REL_SIDEBAR: Color = Color::new([0.09, 0.09, 0.11, 1.0]);
const REL_HOVER: Color = Color::new([0.13, 0.13, 0.17, 1.0]);
const REL_ACTIVE: Color = Color::new([0.16, 0.18, 0.28, 1.0]);
const TAG_COLOR: Color = Color::new([0.45, 0.80, 0.50, 1.0]);
const PRERELEASE_TAG: Color = Color::new([0.85, 0.65, 0.25, 1.0]);
const TIMELINE_COLOR: Color = Color::new([0.30, 0.50, 0.80, 1.0]);
const CHANGE_ADD: Color = Color::new([0.40, 0.80, 0.45, 1.0]);
const CHANGE_FIX: Color = Color::new([0.45, 0.65, 0.95, 1.0]);
const CHANGE_BREAK: Color = Color::new([0.85, 0.35, 0.30, 1.0]);

struct Release {
    version: String,
    date: String,
    is_prerelease: bool,
    changes: Vec<Change>,
}

struct Change {
    kind: ChangeKind,
    description: String,
}

#[derive(Clone, Copy)]
enum ChangeKind {
    Added,
    Fixed,
    Breaking,
}

impl ChangeKind {
    fn color(self) -> Color {
        match self {
            ChangeKind::Added => CHANGE_ADD,
            ChangeKind::Fixed => CHANGE_FIX,
            ChangeKind::Breaking => CHANGE_BREAK,
        }
    }
    fn label(self) -> &'static str {
        match self {
            ChangeKind::Added => "Added",
            ChangeKind::Fixed => "Fixed",
            ChangeKind::Breaking => "Breaking",
        }
    }
}

pub struct ReleasesSpace {
    releases: Vec<Release>,
    selected: usize,
    hovered: Option<usize>,
    scroll_y: f64,
    detail_scroll_y: f64,
}

impl ReleasesSpace {
    pub fn new() -> Self {
        Self {
            releases: vec![
                Release {
                    version: "v0.3.0".into(), date: "Feb 28, 2026".into(), is_prerelease: false,
                    changes: vec![
                        Change { kind: ChangeKind::Added, description: "Plugin system: native (.dylib) + WASM (.wasm)".into() },
                        Change { kind: ChangeKind::Added, description: "SpaceLoader: scan ~/.construct/spaces/".into() },
                        Change { kind: ChangeKind::Added, description: "11 built-in spaces: Notes, Paint, Terminal, etc.".into() },
                        Change { kind: ChangeKind::Fixed, description: "Space bar icon rendering for unknown spaces".into() },
                        Change { kind: ChangeKind::Breaking, description: "ActiveSpace replaced with LoadedSpace enum".into() },
                    ],
                },
                Release {
                    version: "v0.2.0".into(), date: "Feb 25, 2026".into(), is_prerelease: false,
                    changes: vec![
                        Change { kind: ChangeKind::Added, description: "Design space with shapes, layers, properties".into() },
                        Change { kind: ChangeKind::Added, description: "Code space with tree-sitter syntax highlighting".into() },
                        Change { kind: ChangeKind::Added, description: "Space bar navigation with icons".into() },
                    ],
                },
                Release {
                    version: "v0.1.0".into(), date: "Feb 20, 2026".into(), is_prerelease: true,
                    changes: vec![
                        Change { kind: ChangeKind::Added, description: "Initial vello + winit scaffold".into() },
                        Change { kind: ChangeKind::Added, description: "Basic rendering pipeline with wgpu".into() },
                        Change { kind: ChangeKind::Added, description: "Font loading (SF Pro + SF Mono)".into() },
                    ],
                },
            ],
            selected: 0,
            hovered: None,
            scroll_y: 0.0,
            detail_scroll_y: 0.0,
        }
    }
}

impl Space for ReleasesSpace {
    fn name(&self) -> &str {
        "Releases"
    }

    fn draw(
        &mut self,
        scene: &mut Scene,
        xf: Affine,
        area: Rect,
        font: &FontData,
        _mono_font: &FontData,
    ) {
        let ax = area.x0;
        let ay = area.y0;
        let ww = area.width();
        let wh = area.height();

        // Background
        scene.fill(Fill::NonZero, xf, REL_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Header
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + ww, ay + HEADER_H));
        draw_text(scene, font, "Releases", ax + PAD, ay + HEADER_H * 0.66, FONT_SIZE + 2.0, TEXT_COLOR, xf);
        draw_text(scene, font, &format!("{} releases", self.releases.len()), ax + ww - 100.0, ay + HEADER_H * 0.66, SMALL_SIZE, TEXT_SECONDARY, xf);
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax, ay + HEADER_H - 1.0, ax + ww, ay + HEADER_H));

        // Sidebar: release list with timeline
        scene.fill(Fill::NonZero, xf, REL_SIDEBAR, None, &Rect::new(ax, ay + HEADER_H, ax + SIDEBAR_W, ay + wh));

        for (i, rel) in self.releases.iter().enumerate() {
            let y = ay + HEADER_H + i as f64 * RELEASE_ROW_H - self.scroll_y;
            if y + RELEASE_ROW_H < ay + HEADER_H || y > ay + wh { continue; }

            let bg = if i == self.selected {
                REL_ACTIVE
            } else if self.hovered == Some(i) {
                REL_HOVER
            } else {
                REL_SIDEBAR
            };
            scene.fill(Fill::NonZero, xf, bg, None, &Rect::new(ax, y, ax + SIDEBAR_W, y + RELEASE_ROW_H));

            // Timeline dot + line
            let dot_x = ax + 20.0;
            let dot_y = y + RELEASE_ROW_H / 2.0;
            scene.fill(Fill::NonZero, xf, TIMELINE_COLOR, None, &Circle::new((dot_x, dot_y), 5.0));
            if i + 1 < self.releases.len() {
                scene.stroke(&Stroke::new(1.5), xf, TIMELINE_COLOR, None, &Line::new((dot_x, dot_y + 5.0), (dot_x, y + RELEASE_ROW_H)));
            }

            // Version tag
            let tag_color = if rel.is_prerelease { PRERELEASE_TAG } else { TAG_COLOR };
            let tag_w = rel.version.len() as f64 * 7.0 + 12.0;
            let tag_rect = Rect::new(ax + 36.0, y + 12.0, ax + 36.0 + tag_w, y + 28.0);
            scene.fill(Fill::NonZero, xf, tag_color, None, &RoundedRect::from_rect(tag_rect, 4.0));
            draw_text(scene, font, &rel.version, ax + 42.0, y + 25.0, SMALL_SIZE, Color::new([0.05, 0.05, 0.05, 1.0]), xf);

            // Date
            draw_text(scene, font, &rel.date, ax + 36.0, y + 48.0, SMALL_SIZE, TEXT_SECONDARY, xf);

            // Change count
            let count = format!("{} changes", rel.changes.len());
            draw_text(scene, font, &count, ax + SIDEBAR_W - 80.0, y + 48.0, SMALL_SIZE - 1.0, TEXT_SECONDARY, xf);
        }

        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + SIDEBAR_W - 1.0, ay + HEADER_H, ax + SIDEBAR_W, ay + wh));

        // Detail area
        let detail_x = ax + SIDEBAR_W + PAD;
        let rel = &self.releases[self.selected];

        let mut dy = ay + HEADER_H + PAD - self.detail_scroll_y;

        // Version heading
        let tag_color = if rel.is_prerelease { PRERELEASE_TAG } else { TAG_COLOR };
        draw_text(scene, font, &rel.version, detail_x, dy + HEADING_SIZE, HEADING_SIZE, tag_color, xf);
        draw_text(scene, font, &rel.date, detail_x + rel.version.len() as f64 * 12.0 + 16.0, dy + HEADING_SIZE, FONT_SIZE, TEXT_SECONDARY, xf);
        if rel.is_prerelease {
            draw_text(scene, font, "pre-release", detail_x + rel.version.len() as f64 * 12.0 + 130.0, dy + HEADING_SIZE, SMALL_SIZE, PRERELEASE_TAG, xf);
        }
        dy += HEADING_SIZE + 24.0;

        // Changes list
        for change in &rel.changes {
            if dy + 28.0 >= ay + HEADER_H && dy < ay + wh {
                // Tag
                let tag_rect = Rect::new(detail_x, dy, detail_x + change.kind.label().len() as f64 * 6.5 + 10.0, dy + 18.0);
                scene.fill(Fill::NonZero, xf, change.kind.color(), None, &RoundedRect::from_rect(tag_rect, 3.0));
                draw_text(scene, font, change.kind.label(), detail_x + 5.0, dy + 14.0, SMALL_SIZE - 1.0, Color::new([0.05, 0.05, 0.05, 1.0]), xf);

                // Description
                let desc_x = detail_x + change.kind.label().len() as f64 * 6.5 + 20.0;
                draw_text(scene, font, &change.description, desc_x, dy + 14.0, FONT_SIZE, TEXT_COLOR, xf);
            }
            dy += 32.0;
        }
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left { return; }
        let mx = x - area.x0;
        let my = y - area.y0;

        if mx < SIDEBAR_W && my > HEADER_H {
            let idx = ((my - HEADER_H + self.scroll_y) / RELEASE_ROW_H) as usize;
            if idx < self.releases.len() {
                self.selected = idx;
                self.detail_scroll_y = 0.0;
            }
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        if mx < SIDEBAR_W && my > HEADER_H {
            let idx = ((my - HEADER_H + self.scroll_y) / RELEASE_ROW_H) as usize;
            let new_hover = if idx < self.releases.len() { Some(idx) } else { None };
            if new_hover != self.hovered {
                self.hovered = new_hover;
                return true;
            }
        } else if self.hovered.is_some() {
            self.hovered = None;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, mouse_x: f64, _mouse_y: f64, area: Rect) {
        let mx = mouse_x - area.x0;
        if mx < SIDEBAR_W {
            self.scroll_y = (self.scroll_y - dy).max(0.0);
        } else {
            self.detail_scroll_y = (self.detail_scroll_y - dy).max(0.0);
        }
    }

    fn handle_key(&mut self, _key: &Key, _modifiers: ModifiersState) -> bool {
        false
    }

    fn handle_char(&mut self, _ch: &str, _modifiers: ModifiersState) -> bool {
        false
    }
}
