#![allow(dead_code)]

use vello::kurbo::{Affine, BezPath, Circle, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const TOOLBAR_H: f64 = 48.0;
const COLOR_SIZE: f64 = 24.0;
const COLOR_GAP: f64 = 6.0;
const COLOR_PAD: f64 = 12.0;

const PAINT_BG: Color = Color::new([0.95, 0.95, 0.93, 1.0]);

const PALETTE: &[Color] = &[
    Color::new([0.1, 0.1, 0.1, 1.0]),       // black
    Color::new([0.85, 0.2, 0.2, 1.0]),      // red
    Color::new([0.2, 0.65, 0.2, 1.0]),      // green
    Color::new([0.2, 0.4, 0.9, 1.0]),       // blue
    Color::new([0.95, 0.75, 0.1, 1.0]),     // yellow
    Color::new([0.7, 0.3, 0.85, 1.0]),      // purple
    Color::new([0.95, 0.55, 0.1, 1.0]),     // orange
    Color::new([1.0, 1.0, 1.0, 1.0]),       // white
];

#[derive(Clone)]
struct StrokeData {
    points: Vec<Point>,
    color: Color,
    width: f64,
}

pub struct PaintSpace {
    strokes: Vec<StrokeData>,
    current_stroke: Option<StrokeData>,
    selected_color: usize,
    brush_size: f64,
    is_drawing: bool,
}

impl PaintSpace {
    pub fn new() -> Self {
        Self {
            strokes: Vec::new(),
            current_stroke: None,
            selected_color: 0,
            brush_size: 3.0,
            is_drawing: false,
        }
    }
}

impl Space for PaintSpace {
    fn name(&self) -> &str {
        "Paint"
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

        // Canvas background (white-ish)
        scene.fill(Fill::NonZero, xf, PAINT_BG, None, &Rect::new(ax, ay + TOOLBAR_H, ax + ww, ay + wh));

        // Toolbar
        scene.fill(Fill::NonZero, xf, PANEL_BG, None, &Rect::new(ax, ay, ax + ww, ay + TOOLBAR_H));
        scene.fill(Fill::NonZero, xf, PANEL_BORDER, None, &Rect::new(ax, ay + TOOLBAR_H - 1.0, ax + ww, ay + TOOLBAR_H));

        // Color palette
        let mut cx = ax + COLOR_PAD;
        let cy = ay + (TOOLBAR_H - COLOR_SIZE) / 2.0;
        for (i, &color) in PALETTE.iter().enumerate() {
            let r = Rect::new(cx, cy, cx + COLOR_SIZE, cy + COLOR_SIZE);
            scene.fill(Fill::NonZero, xf, color, None, &RoundedRect::from_rect(r, 4.0));
            if i == self.selected_color {
                scene.stroke(&Stroke::new(2.0), xf, SELECT_BLUE, None, &RoundedRect::from_rect(r.inflate(2.0, 2.0), 5.0));
            }
            cx += COLOR_SIZE + COLOR_GAP;
        }

        // Brush size indicator
        cx += 20.0;
        draw_text(scene, font, "Size:", cx, ay + TOOLBAR_H * 0.65, 12.0, TEXT_SECONDARY, xf);
        cx += 40.0;
        let indicator_r = self.brush_size.min(12.0);
        scene.fill(Fill::NonZero, xf, PALETTE[self.selected_color], None, &Circle::new((cx + 12.0, ay + TOOLBAR_H / 2.0), indicator_r));
        cx += 30.0;
        draw_text(scene, font, &format!("{:.0}px", self.brush_size), cx, ay + TOOLBAR_H * 0.65, 12.0, TEXT_COLOR, xf);

        // Clear button
        let clear_x = ax + ww - 80.0;
        let clear_r = Rect::new(clear_x, ay + 8.0, clear_x + 64.0, ay + TOOLBAR_H - 8.0);
        scene.fill(Fill::NonZero, xf, BTN_IDLE, None, &RoundedRect::from_rect(clear_r, 6.0));
        draw_text(scene, font, "Clear", clear_x + 12.0, ay + TOOLBAR_H * 0.65, 12.0, TEXT_COLOR, xf);

        // Draw all strokes
        for stroke in &self.strokes {
            Self::draw_stroke(scene, xf, stroke);
        }
        if let Some(ref current) = self.current_stroke {
            Self::draw_stroke(scene, xf, current);
        }
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left {
            return;
        }
        let mx = x - area.x0;
        let my = y - area.y0;
        let ww = area.width();

        // Toolbar clicks
        if my < TOOLBAR_H {
            // Color palette
            let mut cx = COLOR_PAD;
            for i in 0..PALETTE.len() {
                if mx >= cx && mx < cx + COLOR_SIZE {
                    self.selected_color = i;
                    return;
                }
                cx += COLOR_SIZE + COLOR_GAP;
            }
            // Clear button
            if mx >= ww - 80.0 && mx <= ww - 16.0 {
                self.strokes.clear();
                return;
            }
            return;
        }

        // Start drawing
        self.is_drawing = true;
        self.current_stroke = Some(StrokeData {
            points: vec![Point::new(x, y)],
            color: PALETTE[self.selected_color],
            width: self.brush_size,
        });
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {
        if self.is_drawing {
            if let Some(stroke) = self.current_stroke.take() {
                if stroke.points.len() > 1 {
                    self.strokes.push(stroke);
                }
            }
            self.is_drawing = false;
        }
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, _area: Rect) -> bool {
        if self.is_drawing {
            if let Some(ref mut stroke) = self.current_stroke {
                stroke.points.push(Point::new(x, y));
                return true;
            }
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, _mouse_x: f64, _mouse_y: f64, _area: Rect) {
        self.brush_size = (self.brush_size + dy * 0.1).clamp(1.0, 50.0);
    }

    fn handle_key(&mut self, key: &Key, modifiers: ModifiersState) -> bool {
        let cmd = modifiers.super_key() || modifiers.control_key();
        if let Key::Character(ch) = key {
            if cmd && ch.as_str() == "z" {
                self.strokes.pop();
                return true;
            }
        }
        false
    }

    fn handle_char(&mut self, _ch: &str, _modifiers: ModifiersState) -> bool {
        false
    }
}

impl PaintSpace {
    fn draw_stroke(scene: &mut Scene, xf: Affine, stroke: &StrokeData) {
        if stroke.points.len() < 2 {
            return;
        }
        let mut path = BezPath::new();
        path.move_to(stroke.points[0]);
        for p in &stroke.points[1..] {
            path.line_to(*p);
        }
        scene.stroke(
            &Stroke::new(stroke.width).with_caps(vello::kurbo::Cap::Round).with_join(vello::kurbo::Join::Round),
            xf,
            stroke.color,
            None,
            &path,
        );
    }
}
