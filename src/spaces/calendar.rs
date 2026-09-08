#![allow(dead_code)]

use vello::kurbo::{Affine, Circle, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 56.0;
const DAY_HEADER_H: f64 = 32.0;
const CELL_PAD: f64 = 2.0;
const FONT_SIZE: f64 = 13.0;
const DAY_FONT_SIZE: f64 = 11.0;
const TITLE_SIZE: f64 = 18.0;

const CAL_BG: Color = Color::new([0.11, 0.11, 0.14, 1.0]);
const CELL_BG: Color = Color::new([0.13, 0.13, 0.16, 1.0]);
const CELL_TODAY: Color = Color::new([0.18, 0.22, 0.38, 1.0]);
const CELL_HOVER: Color = Color::new([0.16, 0.16, 0.20, 1.0]);
const DAY_LABEL: Color = Color::new([0.50, 0.50, 0.56, 1.0]);
const DAY_NUM: Color = Color::new([0.80, 0.80, 0.84, 1.0]);
const DAY_OTHER_MONTH: Color = Color::new([0.35, 0.35, 0.40, 1.0]);
const TODAY_ACCENT: Color = Color::new([0.30, 0.55, 0.95, 1.0]);
const EVENT_DOT: Color = Color::new([0.45, 0.80, 0.50, 1.0]);

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

// Simple date math (no external dep)
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

// Zeller's formula for day of week (0=Sun)
fn day_of_week(year: i32, month: u32, day: u32) -> u32 {
    let mut y = year;
    let mut m = month as i32;
    if m < 3 {
        m += 12;
        y -= 1;
    }
    let q = day as i32;
    let k = y % 100;
    let j = y / 100;
    let h = (q + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
    ((h + 6) % 7) as u32 // 0=Sun
}

fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January", 2 => "February", 3 => "March",
        4 => "April", 5 => "May", 6 => "June",
        7 => "July", 8 => "August", 9 => "September",
        10 => "October", 11 => "November", 12 => "December",
        _ => "Unknown",
    }
}

struct Event {
    day: u32,
    title: String,
}

pub struct CalendarSpace {
    year: i32,
    month: u32,
    today_day: u32,
    today_month: u32,
    today_year: i32,
    hovered_cell: Option<(usize, usize)>, // (row, col)
    events: Vec<Event>,
}

impl CalendarSpace {
    pub fn new() -> Self {
        // Hardcoded "today" since no chrono dep
        Self {
            year: 2026,
            month: 2,
            today_day: 28,
            today_month: 2,
            today_year: 2026,
            hovered_cell: None,
            events: vec![
                Event { day: 1, title: "Sprint start".to_string() },
                Event { day: 14, title: "Valentine's Day".to_string() },
                Event { day: 28, title: "Release day".to_string() },
            ],
        }
    }

    fn prev_month(&mut self) {
        if self.month == 1 {
            self.month = 12;
            self.year -= 1;
        } else {
            self.month -= 1;
        }
    }

    fn next_month(&mut self) {
        if self.month == 12 {
            self.month = 1;
            self.year += 1;
        } else {
            self.month += 1;
        }
    }

    fn has_event(&self, day: u32) -> bool {
        self.events.iter().any(|e| e.day == day)
    }
}

impl Space for CalendarSpace {
    fn name(&self) -> &str {
        "Calendar"
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
        scene.fill(Fill::NonZero, xf, CAL_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Header: < February 2026 >
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + ww, ay + HEADER_H));
        let title = format!("{} {}", month_name(self.month), self.year);
        let title_x = ax + ww / 2.0 - title.len() as f64 * 5.0;
        draw_text(scene, font, &title, title_x, ay + HEADER_H * 0.65, TITLE_SIZE, TEXT_COLOR, xf);

        // Nav arrows
        draw_text(scene, font, "<", ax + 20.0, ay + HEADER_H * 0.65, TITLE_SIZE, TEXT_SECONDARY, xf);
        draw_text(scene, font, ">", ax + ww - 30.0, ay + HEADER_H * 0.65, TITLE_SIZE, TEXT_SECONDARY, xf);

        // Day of week headers
        let grid_x = ax + 16.0;
        let grid_w = ww - 32.0;
        let cell_w = grid_w / 7.0;
        let grid_y = ay + HEADER_H;

        for (i, day) in DAYS.iter().enumerate() {
            let x = grid_x + i as f64 * cell_w + cell_w / 2.0 - 10.0;
            draw_text(scene, font, day, x, grid_y + DAY_HEADER_H * 0.72, DAY_FONT_SIZE, DAY_LABEL, xf);
        }

        // Calendar grid
        let start_dow = day_of_week(self.year, self.month, 1);
        let days = days_in_month(self.year, self.month);
        let cell_h = (wh - HEADER_H - DAY_HEADER_H - 16.0) / 6.0;
        let grid_top = grid_y + DAY_HEADER_H;
        let is_today_month = self.year == self.today_year && self.month == self.today_month;

        // Previous month days
        let prev_days = if self.month == 1 {
            days_in_month(self.year - 1, 12)
        } else {
            days_in_month(self.year, self.month - 1)
        };

        let mut day = 1i32 - start_dow as i32;
        for row in 0..6 {
            for col in 0..7 {
                let cx = grid_x + col as f64 * cell_w + CELL_PAD;
                let cy = grid_top + row as f64 * cell_h + CELL_PAD;
                let cw = cell_w - CELL_PAD * 2.0;
                let ch = cell_h - CELL_PAD * 2.0;
                let cell_rect = Rect::new(cx, cy, cx + cw, cy + ch);

                let (display_day, in_month) = if day < 1 {
                    ((prev_days as i32 + day) as u32, false)
                } else if day > days as i32 {
                    ((day - days as i32) as u32, false)
                } else {
                    (day as u32, true)
                };

                let is_today = in_month && is_today_month && display_day == self.today_day;
                let is_hovered = self.hovered_cell == Some((row, col));

                let bg = if is_today {
                    CELL_TODAY
                } else if is_hovered && in_month {
                    CELL_HOVER
                } else {
                    CELL_BG
                };
                scene.fill(Fill::NonZero, xf, bg, None, &RoundedRect::from_rect(cell_rect, 4.0));

                let text_color = if !in_month {
                    DAY_OTHER_MONTH
                } else if is_today {
                    TODAY_ACCENT
                } else {
                    DAY_NUM
                };

                draw_text(scene, font, &format!("{}", display_day), cx + 6.0, cy + 18.0, FONT_SIZE, text_color, xf);

                // Event dot
                if in_month && self.has_event(display_day) {
                    scene.fill(Fill::NonZero, xf, EVENT_DOT, None, &Circle::new((cx + cw - 8.0, cy + 10.0), 3.0));
                }

                day += 1;
            }
        }
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left {
            return;
        }
        let mx = x - area.x0;
        let my = y - area.y0;
        let ww = area.width();

        if my < HEADER_H {
            if mx < 50.0 {
                self.prev_month();
            } else if mx > ww - 50.0 {
                self.next_month();
            }
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        let ww = area.width();
        let wh = area.height();

        let grid_x = 16.0;
        let grid_w = ww - 32.0;
        let cell_w = grid_w / 7.0;
        let grid_top = HEADER_H + DAY_HEADER_H;
        let cell_h = (wh - HEADER_H - DAY_HEADER_H - 16.0) / 6.0;

        if my >= grid_top && mx >= grid_x && mx < grid_x + grid_w {
            let col = ((mx - grid_x) / cell_w) as usize;
            let row = ((my - grid_top) / cell_h) as usize;
            if col < 7 && row < 6 {
                let new_hover = Some((row, col));
                if new_hover != self.hovered_cell {
                    self.hovered_cell = new_hover;
                    return true;
                }
            }
        } else if self.hovered_cell.is_some() {
            self.hovered_cell = None;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, _dy: f64, _mouse_x: f64, _mouse_y: f64, _area: Rect) {}

    fn handle_key(&mut self, key: &Key, _modifiers: ModifiersState) -> bool {
        match key {
            Key::Named(NamedKey::ArrowLeft) => { self.prev_month(); true }
            Key::Named(NamedKey::ArrowRight) => { self.next_month(); true }
            _ => false,
        }
    }

    fn handle_char(&mut self, _ch: &str, _modifiers: ModifiersState) -> bool {
        false
    }
}
