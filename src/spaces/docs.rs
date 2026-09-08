#![allow(dead_code)]

use vello::kurbo::{Affine, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const SIDEBAR_W: f64 = 240.0;
const HEADER_H: f64 = 44.0;
const NAV_ITEM_H: f64 = 32.0;
const CONTENT_PAD: f64 = 32.0;
const FONT_SIZE: f64 = 14.0;
const HEADING_SIZE: f64 = 22.0;
const H2_SIZE: f64 = 16.0;
const SMALL_SIZE: f64 = 12.0;
const LINE_H: f64 = 24.0;

const DOCS_BG: Color = Color::new([0.11, 0.11, 0.14, 1.0]);
const DOCS_SIDEBAR: Color = Color::new([0.09, 0.09, 0.12, 1.0]);
const NAV_HOVER: Color = Color::new([0.13, 0.13, 0.17, 1.0]);
const NAV_ACTIVE: Color = Color::new([0.16, 0.18, 0.28, 1.0]);
const HEADING_COLOR: Color = Color::new([0.92, 0.92, 0.95, 1.0]);
const BODY_COLOR: Color = Color::new([0.78, 0.78, 0.82, 1.0]);
const LINK_COLOR: Color = Color::new([0.40, 0.65, 0.95, 1.0]);
const CODE_BG_INLINE: Color = Color::new([0.15, 0.15, 0.19, 1.0]);
const ACCENT_DOCS: Color = Color::new([0.35, 0.75, 0.55, 1.0]);

struct DocPage {
    title: &'static str,
    sections: Vec<DocSection>,
}

struct DocSection {
    heading: &'static str,
    body: Vec<&'static str>,
}

struct NavItem {
    label: &'static str,
    indent: usize,
}

pub struct DocsSpace {
    pages: Vec<DocPage>,
    nav_items: Vec<NavItem>,
    active_page: usize,
    hovered_nav: Option<usize>,
    scroll_y: f64,
    sidebar_scroll_y: f64,
}

impl DocsSpace {
    pub fn new() -> Self {
        let pages = vec![
            DocPage {
                title: "Getting Started",
                sections: vec![
                    DocSection {
                        heading: "Installation",
                        body: vec![
                            "Install Construct from source:",
                            "",
                            "  cargo install construct-rust",
                            "",
                            "Or clone and build:",
                            "",
                            "  git clone https://github.com/construct/construct",
                            "  cd construct-rust",
                            "  cargo build --release",
                        ],
                    },
                    DocSection {
                        heading: "First Run",
                        body: vec![
                            "Run Construct to see the default spaces:",
                            "",
                            "  cargo run",
                            "",
                            "The app opens with Design and Code spaces in the sidebar.",
                            "Click the icons to switch between spaces.",
                        ],
                    },
                ],
            },
            DocPage {
                title: "Plugin System",
                sections: vec![
                    DocSection {
                        heading: "Overview",
                        body: vec![
                            "Construct supports two types of plugins:",
                            "",
                            "Native plugins (.dylib/.so/.dll) get direct Scene access",
                            "for zero-cost rendering with full GPU power.",
                            "",
                            "WASM plugins (.wasm) run in a sandboxed wasmtime runtime",
                            "and call host functions for rendering.",
                        ],
                    },
                    DocSection {
                        heading: "Creating a Native Plugin",
                        body: vec![
                            "Create a new Rust crate with crate-type = [\"cdylib\"].",
                            "Export the required C ABI functions:",
                            "",
                            "  construct_space_create() -> *mut c_void",
                            "  construct_space_destroy(ptr)",
                            "  construct_space_name(ptr) -> *const c_char",
                            "  construct_space_draw(ptr, scene, xf, area, ...)",
                        ],
                    },
                ],
            },
            DocPage {
                title: "Space Trait",
                sections: vec![
                    DocSection {
                        heading: "The Space Interface",
                        body: vec![
                            "Every space implements the Space trait:",
                            "",
                            "  fn name() -> &str",
                            "  fn draw(scene, xf, area, font, mono_font)",
                            "  fn handle_mouse_click(x, y, button, area)",
                            "  fn handle_mouse_move(x, y, area) -> bool",
                            "  fn handle_scroll(dx, dy, mx, my, area)",
                            "  fn handle_key(key, modifiers) -> bool",
                            "  fn handle_char(ch, modifiers) -> bool",
                        ],
                    },
                ],
            },
        ];

        let nav_items = vec![
            NavItem { label: "Getting Started", indent: 0 },
            NavItem { label: "Installation", indent: 1 },
            NavItem { label: "First Run", indent: 1 },
            NavItem { label: "Plugin System", indent: 0 },
            NavItem { label: "Overview", indent: 1 },
            NavItem { label: "Creating a Native Plugin", indent: 1 },
            NavItem { label: "Space Trait", indent: 0 },
            NavItem { label: "The Space Interface", indent: 1 },
        ];

        Self {
            pages,
            nav_items,
            active_page: 0,
            hovered_nav: None,
            scroll_y: 0.0,
            sidebar_scroll_y: 0.0,
        }
    }
}

impl Space for DocsSpace {
    fn name(&self) -> &str {
        "Docs"
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
        scene.fill(Fill::NonZero, xf, DOCS_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Sidebar
        scene.fill(Fill::NonZero, xf, DOCS_SIDEBAR, None, &Rect::new(ax, ay, ax + SIDEBAR_W, ay + wh));

        // Sidebar header
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + SIDEBAR_W, ay + HEADER_H));
        draw_text(scene, font, "Documentation", ax + 12.0, ay + HEADER_H * 0.66, FONT_SIZE, ACCENT_DOCS, xf);

        // Nav items
        for (i, item) in self.nav_items.iter().enumerate() {
            let y = ay + HEADER_H + i as f64 * NAV_ITEM_H - self.sidebar_scroll_y;
            if y + NAV_ITEM_H < ay + HEADER_H || y > ay + wh { continue; }

            let is_page_header = item.indent == 0;
            // Map nav items to page indices
            let page_idx = self.nav_items[..=i].iter().filter(|n| n.indent == 0).count().saturating_sub(1);
            let is_active = page_idx == self.active_page && is_page_header;

            let bg = if is_active {
                NAV_ACTIVE
            } else if self.hovered_nav == Some(i) {
                NAV_HOVER
            } else {
                DOCS_SIDEBAR
            };
            scene.fill(Fill::NonZero, xf, bg, None, &Rect::new(ax, y, ax + SIDEBAR_W, y + NAV_ITEM_H));

            let indent = 12.0 + item.indent as f64 * 16.0;
            let color = if is_page_header { TEXT_COLOR } else { TEXT_SECONDARY };
            let size = if is_page_header { FONT_SIZE } else { SMALL_SIZE };
            draw_text(scene, font, item.label, ax + indent, y + NAV_ITEM_H * 0.7, size, color, xf);
        }

        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + SIDEBAR_W - 1.0, ay, ax + SIDEBAR_W, ay + wh));

        // Content area
        let content_x = ax + SIDEBAR_W + CONTENT_PAD;
        let page = &self.pages[self.active_page];

        let mut y = ay + CONTENT_PAD - self.scroll_y;

        // Page title
        draw_text(scene, font, page.title, content_x, y + HEADING_SIZE, HEADING_SIZE, HEADING_COLOR, xf);
        y += HEADING_SIZE + 20.0;

        // Divider under title
        scene.fill(Fill::NonZero, xf, ACCENT_DOCS, None, &Rect::new(content_x, y, content_x + 60.0, y + 2.0));
        y += 20.0;

        for section in &page.sections {
            // Section heading
            draw_text(scene, font, section.heading, content_x, y + H2_SIZE, H2_SIZE, HEADING_COLOR, xf);
            y += H2_SIZE + 12.0;

            for line in &section.body {
                if y + LINE_H >= ay && y < ay + wh {
                    if line.starts_with("  ") {
                        // Code-like lines
                        let code_rect = Rect::new(content_x - 4.0, y, content_x + line.len() as f64 * 7.5 + 12.0, y + LINE_H);
                        scene.fill(Fill::NonZero, xf, CODE_BG_INLINE, None, &RoundedRect::from_rect(code_rect, 3.0));
                        draw_text(scene, mono_font, line, content_x, y + LINE_H * 0.78, SMALL_SIZE + 1.0, LINK_COLOR, xf);
                    } else if !line.is_empty() {
                        draw_text(scene, font, line, content_x, y + LINE_H * 0.78, FONT_SIZE, BODY_COLOR, xf);
                    }
                }
                y += LINE_H;
            }
            y += 16.0;
        }
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left { return; }
        let mx = x - area.x0;
        let my = y - area.y0;

        if mx < SIDEBAR_W && my > HEADER_H {
            let idx = ((my - HEADER_H + self.sidebar_scroll_y) / NAV_ITEM_H) as usize;
            if idx < self.nav_items.len() && self.nav_items[idx].indent == 0 {
                let page_idx = self.nav_items[..=idx].iter().filter(|n| n.indent == 0).count().saturating_sub(1);
                if page_idx < self.pages.len() {
                    self.active_page = page_idx;
                    self.scroll_y = 0.0;
                }
            }
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        if mx < SIDEBAR_W && my > HEADER_H {
            let idx = ((my - HEADER_H + self.sidebar_scroll_y) / NAV_ITEM_H) as usize;
            let new_hover = if idx < self.nav_items.len() { Some(idx) } else { None };
            if new_hover != self.hovered_nav {
                self.hovered_nav = new_hover;
                return true;
            }
        } else if self.hovered_nav.is_some() {
            self.hovered_nav = None;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, mouse_x: f64, _mouse_y: f64, area: Rect) {
        let mx = mouse_x - area.x0;
        if mx < SIDEBAR_W {
            self.sidebar_scroll_y = (self.sidebar_scroll_y - dy).max(0.0);
        } else {
            self.scroll_y = (self.scroll_y - dy).max(0.0);
        }
    }

    fn handle_key(&mut self, key: &Key, _modifiers: ModifiersState) -> bool {
        match key {
            Key::Named(NamedKey::ArrowUp) => {
                if self.active_page > 0 { self.active_page -= 1; self.scroll_y = 0.0; }
                true
            }
            Key::Named(NamedKey::ArrowDown) => {
                if self.active_page + 1 < self.pages.len() { self.active_page += 1; self.scroll_y = 0.0; }
                true
            }
            _ => false,
        }
    }

    fn handle_char(&mut self, _ch: &str, _modifiers: ModifiersState) -> bool {
        false
    }
}
