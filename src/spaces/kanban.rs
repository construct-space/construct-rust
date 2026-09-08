#![allow(dead_code)]

use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 44.0;
const COL_GAP: f64 = 12.0;
const COL_PAD: f64 = 16.0;
const CARD_H: f64 = 72.0;
const CARD_GAP: f64 = 8.0;
const CARD_PAD: f64 = 10.0;
const FONT_SIZE: f64 = 13.0;
const TITLE_SIZE: f64 = 12.0;
const TAG_SIZE: f64 = 10.0;

const KANBAN_BG: Color = Color::new([0.10, 0.10, 0.13, 1.0]);
const COL_BG: Color = Color::new([0.12, 0.12, 0.15, 1.0]);
const COL_HEADER: Color = Color::new([0.14, 0.14, 0.17, 1.0]);
const CARD_BG: Color = Color::new([0.16, 0.16, 0.20, 1.0]);
const CARD_HOVER: Color = Color::new([0.19, 0.19, 0.24, 1.0]);
const CARD_DRAG: Color = Color::new([0.22, 0.26, 0.42, 1.0]);
const TAG_BUG: Color = Color::new([0.85, 0.30, 0.30, 1.0]);
const TAG_FEATURE: Color = Color::new([0.30, 0.65, 0.95, 1.0]);
const TAG_CHORE: Color = Color::new([0.55, 0.55, 0.60, 1.0]);

#[derive(Clone, Copy, PartialEq)]
enum Tag {
    Bug,
    Feature,
    Chore,
}

impl Tag {
    fn color(self) -> Color {
        match self {
            Tag::Bug => TAG_BUG,
            Tag::Feature => TAG_FEATURE,
            Tag::Chore => TAG_CHORE,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Tag::Bug => "Bug",
            Tag::Feature => "Feature",
            Tag::Chore => "Chore",
        }
    }
}

#[derive(Clone)]
struct Card {
    title: String,
    description: String,
    tag: Tag,
}

struct Column {
    title: &'static str,
    cards: Vec<Card>,
}

pub struct KanbanSpace {
    columns: Vec<Column>,
    hovered_card: Option<(usize, usize)>, // (col, card)
    dragging: Option<(usize, usize)>,
    drag_target_col: Option<usize>,
    scroll_y: f64,
}

impl KanbanSpace {
    pub fn new() -> Self {
        Self {
            columns: vec![
                Column {
                    title: "Todo",
                    cards: vec![
                        Card { title: "Plugin error boundaries".into(), description: "Catch panics in plugins".into(), tag: Tag::Feature },
                        Card { title: "Fix scroll overflow".into(), description: "Scroll past last line".into(), tag: Tag::Bug },
                        Card { title: "Add CI pipeline".into(), description: "GitHub Actions for tests".into(), tag: Tag::Chore },
                    ],
                },
                Column {
                    title: "In Progress",
                    cards: vec![
                        Card { title: "Implement all spaces".into(), description: "Notes, Paint, Terminal...".into(), tag: Tag::Feature },
                        Card { title: "WASM host functions".into(), description: "draw_text, fill_rect bridge".into(), tag: Tag::Feature },
                    ],
                },
                Column {
                    title: "Done",
                    cards: vec![
                        Card { title: "Plugin system".into(), description: "Native + WASM loading".into(), tag: Tag::Feature },
                        Card { title: "Space trait".into(), description: "Core abstraction done".into(), tag: Tag::Chore },
                    ],
                },
            ],
            hovered_card: None,
            dragging: None,
            drag_target_col: None,
            scroll_y: 0.0,
        }
    }
}

impl Space for KanbanSpace {
    fn name(&self) -> &str {
        "Kanban"
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
        scene.fill(Fill::NonZero, xf, KANBAN_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Header
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + ww, ay + HEADER_H));
        draw_text(scene, font, "Kanban Board", ax + COL_PAD, ay + HEADER_H * 0.66, FONT_SIZE + 2.0, TEXT_COLOR, xf);

        let n_cols = self.columns.len() as f64;
        let col_w = (ww - COL_PAD * 2.0 - COL_GAP * (n_cols - 1.0)) / n_cols;
        let col_top = ay + HEADER_H + COL_PAD;

        for (ci, col) in self.columns.iter().enumerate() {
            let cx = ax + COL_PAD + ci as f64 * (col_w + COL_GAP);

            // Column background
            scene.fill(
                Fill::NonZero,
                xf,
                COL_BG,
                None,
                &RoundedRect::from_rect(Rect::new(cx, col_top, cx + col_w, ay + wh - COL_PAD), 8.0),
            );

            // Column header
            scene.fill(
                Fill::NonZero,
                xf,
                COL_HEADER,
                None,
                &RoundedRect::from_rect(Rect::new(cx, col_top, cx + col_w, col_top + 36.0), 8.0),
            );
            let count_str = format!("{} ({})", col.title, col.cards.len());
            draw_text(scene, font, &count_str, cx + CARD_PAD, col_top + 24.0, TITLE_SIZE, TEXT_COLOR, xf);

            // Drop target highlight
            if self.drag_target_col == Some(ci) {
                scene.stroke(
                    &Stroke::new(2.0),
                    xf,
                    SELECT_BLUE,
                    None,
                    &RoundedRect::from_rect(Rect::new(cx, col_top, cx + col_w, ay + wh - COL_PAD), 8.0),
                );
            }

            // Cards
            let mut card_y = col_top + 44.0;
            for (ki, card) in col.cards.iter().enumerate() {
                let is_hovered = self.hovered_card == Some((ci, ki));
                let is_dragging = self.dragging == Some((ci, ki));
                let bg = if is_dragging {
                    CARD_DRAG
                } else if is_hovered {
                    CARD_HOVER
                } else {
                    CARD_BG
                };

                let card_rect = Rect::new(cx + 6.0, card_y, cx + col_w - 6.0, card_y + CARD_H);
                scene.fill(Fill::NonZero, xf, bg, None, &RoundedRect::from_rect(card_rect, 6.0));

                // Tag
                let tag_rect = Rect::new(cx + 14.0, card_y + 8.0, cx + 14.0 + card.tag.label().len() as f64 * 6.5 + 8.0, card_y + 22.0);
                scene.fill(Fill::NonZero, xf, card.tag.color(), None, &RoundedRect::from_rect(tag_rect, 3.0));
                draw_text(scene, font, card.tag.label(), cx + 18.0, card_y + 19.0, TAG_SIZE, Color::new([1.0, 1.0, 1.0, 1.0]), xf);

                // Title
                let title = if card.title.len() > 30 { format!("{}...", &card.title[..27]) } else { card.title.clone() };
                draw_text(scene, font, &title, cx + 14.0, card_y + 40.0, FONT_SIZE, TEXT_COLOR, xf);

                // Description
                let desc = if card.description.len() > 35 { format!("{}...", &card.description[..32]) } else { card.description.clone() };
                draw_text(scene, font, &desc, cx + 14.0, card_y + 58.0, TAG_SIZE + 1.0, TEXT_SECONDARY, xf);

                card_y += CARD_H + CARD_GAP;
            }
        }
    }

    fn handle_mouse_click(&mut self, _x: f64, _y: f64, button: MouseButton, _area: Rect) {
        if button != MouseButton::Left {
            return;
        }
        // Find which card was clicked
        if let Some((ci, ki)) = self.hovered_card {
            self.dragging = Some((ci, ki));
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {
        // Drop card into target column
        if let (Some((src_col, src_idx)), Some(dst_col)) = (self.dragging, self.drag_target_col) {
            if src_col != dst_col && src_idx < self.columns[src_col].cards.len() {
                let card = self.columns[src_col].cards.remove(src_idx);
                self.columns[dst_col].cards.push(card);
            }
        }
        self.dragging = None;
        self.drag_target_col = None;
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        let ww = area.width();

        let n_cols = self.columns.len() as f64;
        let col_w = (ww - COL_PAD * 2.0 - COL_GAP * (n_cols - 1.0)) / n_cols;
        let col_top = HEADER_H + COL_PAD;

        // Determine which column we're over
        let mut target_col = None;
        for ci in 0..self.columns.len() {
            let col_x = COL_PAD + ci as f64 * (col_w + COL_GAP);
            if mx >= col_x && mx < col_x + col_w && my >= col_top {
                target_col = Some(ci);
                break;
            }
        }

        if self.dragging.is_some() {
            let old = self.drag_target_col;
            self.drag_target_col = target_col;
            return old != self.drag_target_col;
        }

        // Find hovered card
        let mut new_hover = None;
        if let Some(ci) = target_col {
            let mut card_y = col_top + 44.0;
            for ki in 0..self.columns[ci].cards.len() {
                if my >= card_y && my < card_y + CARD_H {
                    new_hover = Some((ci, ki));
                    break;
                }
                card_y += CARD_H + CARD_GAP;
            }
        }

        if new_hover != self.hovered_card {
            self.hovered_card = new_hover;
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
