#![allow(dead_code)]

use std::time::Instant;

use vello::kurbo::{
    Affine, BezPath, Circle, Ellipse, Line, Point, Rect, RoundedRect, Stroke, Vec2,
};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::{draw_text, layout_glyphs};
use crate::theme::*;

use super::Space;

// ── Geometry ────────────────────────────────────────────────────────
const HANDLE_R: f64 = 5.0;
const GRID_SPACING: f64 = 24.0;
const LAYERS_W: f64 = 220.0;
const LAYERS_HEADER_H: f64 = 40.0;
const LAYER_ROW_H: f64 = 32.0;
const PROPS_W: f64 = 240.0;
const PROPS_HEADER_H: f64 = 40.0;
const PROPS_ROW_H: f64 = 28.0;
const PROPS_LABEL_W: f64 = 60.0;
const DOCK_H: f64 = 48.0;
const DOCK_BOTTOM_MARGIN: f64 = 16.0;
const DOCK_RADIUS: f64 = 12.0;
const DOCK_BTN_SIZE: f64 = 32.0;
const DOCK_BTN_GAP: f64 = 4.0;
const DOCK_SWATCH_SIZE: f64 = 22.0;
const DOCK_SWATCH_GAP: f64 = 3.0;
const DOCK_PAD: f64 = 8.0;
const DOCK_DIVIDER_W: f64 = 12.0;
const DEFAULT_FONT_SIZE: f64 = 28.0;

// ── Data model ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Rect,
    Circle,
    Line,
    Freehand,
    Text,
    Artboard,
}

const TOOLS: [Tool; 7] = [
    Tool::Select,
    Tool::Rect,
    Tool::Circle,
    Tool::Line,
    Tool::Freehand,
    Tool::Text,
    Tool::Artboard,
];

#[derive(Debug, Clone)]
enum ShapeKind {
    Rect,
    Circle,
    Line { end: Point },
    Freehand { points: Vec<Point> },
    Text { text: String, font_size: f64 },
    Artboard,
}

#[derive(Debug, Clone)]
struct DrawShape {
    name: String,
    kind: ShapeKind,
    origin: Point,
    size: (f64, f64),
    rotation: f64,
    color: Color,
    stroke_width: f64,
}

#[derive(Debug, Clone, Default)]
struct ShapeCounters {
    rect: u32,
    circle: u32,
    line: u32,
    freehand: u32,
    text: u32,
    artboard: u32,
}

impl ShapeCounters {
    fn next_name(&mut self, kind: &ShapeKind) -> String {
        match kind {
            ShapeKind::Rect => { self.rect += 1; format!("Rectangle {}", self.rect) }
            ShapeKind::Circle => { self.circle += 1; format!("Circle {}", self.circle) }
            ShapeKind::Line { .. } => { self.line += 1; format!("Line {}", self.line) }
            ShapeKind::Freehand { .. } => { self.freehand += 1; format!("Freehand {}", self.freehand) }
            ShapeKind::Text { .. } => { self.text += 1; format!("Text {}", self.text) }
            ShapeKind::Artboard => { self.artboard += 1; format!("Artboard {}", self.artboard) }
        }
    }
}

// ── Geometry helpers ────────────────────────────────────────────────

fn dist_pt_seg(p: Point, a: Point, b: Point) -> f64 {
    let ab = Vec2::new(b.x - a.x, b.y - a.y);
    let ap = Vec2::new(p.x - a.x, p.y - a.y);
    let len2 = ab.dot(ab);
    if len2 < 1e-12 {
        return ap.length();
    }
    let t = (ap.dot(ab) / len2).clamp(0.0, 1.0);
    let proj = Point::new(a.x + t * ab.x, a.y + t * ab.y);
    ((p.x - proj.x).powi(2) + (p.y - proj.y).powi(2)).sqrt()
}

impl DrawShape {
    fn center(&self) -> Point {
        match &self.kind {
            ShapeKind::Rect | ShapeKind::Artboard => Point::new(
                self.origin.x + self.size.0 / 2.0,
                self.origin.y + self.size.1 / 2.0,
            ),
            ShapeKind::Circle => self.origin,
            ShapeKind::Line { end } => Point::new(
                (self.origin.x + end.x) / 2.0,
                (self.origin.y + end.y) / 2.0,
            ),
            ShapeKind::Freehand { .. } | ShapeKind::Text { .. } => {
                let b = self.bounds();
                Point::new((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0)
            }
        }
    }

    fn bounds(&self) -> Rect {
        match &self.kind {
            ShapeKind::Rect | ShapeKind::Artboard => Rect::new(
                self.origin.x,
                self.origin.y,
                self.origin.x + self.size.0,
                self.origin.y + self.size.1,
            ),
            ShapeKind::Circle => {
                let r = self.size.0.abs();
                Rect::new(
                    self.origin.x - r,
                    self.origin.y - r,
                    self.origin.x + r,
                    self.origin.y + r,
                )
            }
            ShapeKind::Line { end } => Rect::new(
                self.origin.x.min(end.x),
                self.origin.y.min(end.y),
                self.origin.x.max(end.x),
                self.origin.y.max(end.y),
            ),
            ShapeKind::Freehand { points } => {
                let (mut x0, mut y0, mut x1, mut y1) =
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for p in points {
                    x0 = x0.min(p.x);
                    y0 = y0.min(p.y);
                    x1 = x1.max(p.x);
                    y1 = y1.max(p.y);
                }
                Rect::new(x0, y0, x1, y1)
            }
            ShapeKind::Text { text, font_size } => {
                let approx_w = text.len() as f64 * font_size * 0.55;
                Rect::new(
                    self.origin.x,
                    self.origin.y - font_size,
                    self.origin.x + approx_w,
                    self.origin.y + font_size * 0.25,
                )
            }
        }
    }

    fn hit_test(&self, pt: Point) -> bool {
        let local = if self.rotation.abs() > 1e-4 {
            let c = self.center();
            let (dx, dy) = (pt.x - c.x, pt.y - c.y);
            let (cos, sin) = ((-self.rotation).cos(), (-self.rotation).sin());
            Point::new(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
        } else {
            pt
        };
        match &self.kind {
            ShapeKind::Rect | ShapeKind::Artboard => self.bounds().contains(local),
            ShapeKind::Circle => {
                let d = ((local.x - self.origin.x).powi(2)
                    + (local.y - self.origin.y).powi(2))
                .sqrt();
                d <= self.size.0.abs()
            }
            ShapeKind::Line { end } => dist_pt_seg(local, self.origin, *end) < 8.0,
            ShapeKind::Freehand { points } => {
                for w in points.windows(2) {
                    if dist_pt_seg(local, w[0], w[1]) < 8.0 {
                        return true;
                    }
                }
                false
            }
            ShapeKind::Text { .. } => self.bounds().contains(local),
        }
    }

    fn translate(&mut self, d: Vec2) {
        self.origin += d;
        match &mut self.kind {
            ShapeKind::Line { end } => *end += d,
            ShapeKind::Freehand { points } => {
                for p in points {
                    *p += d;
                }
            }
            _ => {}
        }
    }
}

fn corner_handles_of(shape: &DrawShape) -> [Point; 4] {
    let b = shape.bounds();
    let raw = [
        Point::new(b.x0, b.y0),
        Point::new(b.x1, b.y0),
        Point::new(b.x1, b.y1),
        Point::new(b.x0, b.y1),
    ];
    if shape.rotation.abs() > 1e-4 {
        let c = shape.center();
        let (cos, sin) = (shape.rotation.cos(), shape.rotation.sin());
        raw.map(|p| {
            let (dx, dy) = (p.x - c.x, p.y - c.y);
            Point::new(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
        })
    } else {
        raw
    }
}

// ── Dock / Panel geometry ───────────────────────────────────────────

fn dock_content_width(has_delete: bool) -> f64 {
    let tools_w = TOOLS.len() as f64 * (DOCK_BTN_SIZE + DOCK_BTN_GAP) - DOCK_BTN_GAP;
    let swatches_w = 8.0 * (DOCK_SWATCH_SIZE + DOCK_SWATCH_GAP) - DOCK_SWATCH_GAP;
    let mut w = DOCK_PAD + tools_w + DOCK_DIVIDER_W + swatches_w + DOCK_PAD;
    if has_delete {
        w += DOCK_DIVIDER_W + DOCK_BTN_SIZE;
    }
    w
}

fn dock_rect(area: Rect, has_delete: bool) -> Rect {
    let w = dock_content_width(has_delete);
    let canvas_center_x = area.x0 + LAYERS_W + (area.width() - LAYERS_W - PROPS_W) / 2.0;
    let x = canvas_center_x - w / 2.0;
    let y = area.y0 + area.height() - DOCK_BOTTOM_MARGIN - DOCK_H;
    Rect::new(x, y, x + w, y + DOCK_H)
}

fn dock_tool_btn_rect(i: usize, area: Rect, has_delete: bool) -> Rect {
    let dr = dock_rect(area, has_delete);
    let x = dr.x0 + DOCK_PAD + i as f64 * (DOCK_BTN_SIZE + DOCK_BTN_GAP);
    let y = dr.y0 + (DOCK_H - DOCK_BTN_SIZE) / 2.0;
    Rect::new(x, y, x + DOCK_BTN_SIZE, y + DOCK_BTN_SIZE)
}

fn dock_swatch_rect(i: usize, area: Rect, has_delete: bool) -> Rect {
    let dr = dock_rect(area, has_delete);
    let tools_w = TOOLS.len() as f64 * (DOCK_BTN_SIZE + DOCK_BTN_GAP) - DOCK_BTN_GAP;
    let swatch_start_x = dr.x0 + DOCK_PAD + tools_w + DOCK_DIVIDER_W;
    let x = swatch_start_x + i as f64 * (DOCK_SWATCH_SIZE + DOCK_SWATCH_GAP);
    let y = dr.y0 + (DOCK_H - DOCK_SWATCH_SIZE) / 2.0;
    Rect::new(x, y, x + DOCK_SWATCH_SIZE, y + DOCK_SWATCH_SIZE)
}

fn dock_delete_rect(area: Rect) -> Rect {
    let dr = dock_rect(area, true);
    let tools_w = TOOLS.len() as f64 * (DOCK_BTN_SIZE + DOCK_BTN_GAP) - DOCK_BTN_GAP;
    let swatches_w = 8.0 * (DOCK_SWATCH_SIZE + DOCK_SWATCH_GAP) - DOCK_SWATCH_GAP;
    let x = dr.x0 + DOCK_PAD + tools_w + DOCK_DIVIDER_W + swatches_w + DOCK_DIVIDER_W;
    let y = dr.y0 + (DOCK_H - DOCK_BTN_SIZE) / 2.0;
    Rect::new(x, y, x + DOCK_BTN_SIZE, y + DOCK_BTN_SIZE)
}

fn layer_row_rect(display_index: usize) -> Rect {
    let y = LAYERS_HEADER_H + display_index as f64 * LAYER_ROW_H;
    Rect::new(0.0, y, LAYERS_W, y + LAYER_ROW_H)
}

fn props_palette_base_y() -> f64 {
    PROPS_HEADER_H + 12.0
        + PROPS_ROW_H * 2.0
        + 8.0
        + PROPS_ROW_H * 2.0
        + 8.0
        + PROPS_ROW_H
        + 8.0
        + PROPS_ROW_H
        + PROPS_ROW_H
        + 12.0
}

fn props_palette_swatch_rect(i: usize, panel_x: f64, base_y: f64) -> Rect {
    let cols = 4;
    let col = i % cols;
    let row = i / cols;
    let size = 24.0;
    let gap = 4.0;
    let x = panel_x + 12.0 + col as f64 * (size + gap);
    let y = base_y + row as f64 * (size + gap);
    Rect::new(x, y, x + size, y + size)
}

// ── DesignSpace ─────────────────────────────────────────────────────

pub struct DesignSpace {
    start: Instant,
    frame: u64,
    tool: Tool,
    shapes: Vec<DrawShape>,
    selected: Option<usize>,
    mouse_logical: Point,
    drag_start: Option<Point>,
    is_dragging: bool,
    current_color: Color,
    palette: Vec<Color>,
    freehand_buf: Vec<Point>,
    moving: bool,
    resize_corner: Option<usize>,
    shape_counters: ShapeCounters,
    hovered_layer: Option<usize>,
    zoom: f64,
    pan: Vec2,
    is_panning: bool,
    pan_anchor: Point,
    pan_anchor_val: Vec2,
    editing_text: Option<usize>,
}

impl DesignSpace {
    pub fn new() -> Self {
        let palette = vec![
            Color::new([0.91, 0.30, 0.24, 1.0]),
            Color::new([0.90, 0.49, 0.13, 1.0]),
            Color::new([0.95, 0.77, 0.06, 1.0]),
            Color::new([0.18, 0.80, 0.44, 1.0]),
            Color::new([0.20, 0.60, 0.86, 1.0]),
            Color::new([0.61, 0.35, 0.71, 1.0]),
            Color::new([0.93, 0.94, 0.95, 1.0]),
            Color::new([0.20, 0.29, 0.37, 1.0]),
        ];
        let current_color = palette[0];
        Self {
            start: Instant::now(),
            frame: 0,
            tool: Tool::Rect,
            shapes: vec![],
            selected: None,
            mouse_logical: Point::ZERO,
            drag_start: None,
            is_dragging: false,
            current_color,
            palette,
            freehand_buf: vec![],
            moving: false,
            resize_corner: None,
            shape_counters: ShapeCounters::default(),
            hovered_layer: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            is_panning: false,
            pan_anchor: Point::ZERO,
            pan_anchor_val: Vec2::ZERO,
            editing_text: None,
        }
    }

    fn to_canvas(&self, logical: Point, area: Rect) -> Point {
        Point::new(
            (logical.x - area.x0 - LAYERS_W - self.pan.x) / self.zoom,
            (logical.y - area.y0 - self.pan.y) / self.zoom,
        )
    }

    fn find_shape_at(&self, canvas_pos: Point) -> Option<usize> {
        self.shapes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, s)| s.hit_test(canvas_pos).then_some(i))
    }

    fn hit_handle(&self, canvas_pos: Point) -> Option<usize> {
        let idx = self.selected?;
        if idx >= self.shapes.len() {
            return None;
        }
        let shape = &self.shapes[idx];
        let hs = corner_handles_of(shape);
        let threshold = (HANDLE_R + 4.0) / self.zoom;

        let b = shape.bounds();
        let midpoints = [
            Point::new((b.x0 + b.x1) / 2.0, b.y0),
            Point::new(b.x1, (b.y0 + b.y1) / 2.0),
            Point::new((b.x0 + b.x1) / 2.0, b.y1),
            Point::new(b.x0, (b.y0 + b.y1) / 2.0),
        ];

        for (i, h) in hs.iter().enumerate() {
            let d = ((canvas_pos.x - h.x).powi(2) + (canvas_pos.y - h.y).powi(2)).sqrt();
            if d < threshold {
                return Some(i);
            }
        }
        for (i, mp) in midpoints.iter().enumerate() {
            let mp_rot = if shape.rotation.abs() > 1e-4 {
                let c = shape.center();
                let (cos, sin) = (shape.rotation.cos(), shape.rotation.sin());
                let (dx, dy) = (mp.x - c.x, mp.y - c.y);
                Point::new(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
            } else {
                *mp
            };
            let d = ((canvas_pos.x - mp_rot.x).powi(2) + (canvas_pos.y - mp_rot.y).powi(2)).sqrt();
            if d < threshold {
                return Some(4 + i);
            }
        }
        None
    }

    fn set_palette_color(&mut self, i: usize) {
        if i < self.palette.len() {
            self.current_color = self.palette[i];
            if let Some(idx) = self.selected {
                if idx < self.shapes.len() {
                    self.shapes[idx].color = self.current_color;
                }
            }
        }
    }

    fn zoom_at(&mut self, logical_pos: Point, factor: f64, area: Rect) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * factor).clamp(0.05, 64.0);
        let cvx = logical_pos.x - area.x0 - LAYERS_W;
        let cvy = logical_pos.y - area.y0;
        self.pan.x = cvx - (cvx - self.pan.x) * self.zoom / old_zoom;
        self.pan.y = cvy - (cvy - self.pan.y) * self.zoom / old_zoom;
    }

    fn handle_dock_click(&mut self, pos: Point, area: Rect) -> bool {
        let has_del = self.selected.is_some();
        for (i, t) in TOOLS.iter().enumerate() {
            if dock_tool_btn_rect(i, area, has_del).contains(pos) {
                self.tool = *t;
                return true;
            }
        }
        for i in 0..self.palette.len() {
            if dock_swatch_rect(i, area, has_del).contains(pos) {
                self.set_palette_color(i);
                return true;
            }
        }
        if has_del && dock_delete_rect(area).contains(pos) {
            if let Some(idx) = self.selected.take() {
                if idx < self.shapes.len() {
                    self.shapes.remove(idx);
                }
            }
            return true;
        }
        false
    }

    fn handle_layers_click(&mut self, pos: Point, area: Rect) -> bool {
        let local_y = pos.y - area.y0;
        if local_y < LAYERS_HEADER_H {
            return false;
        }
        let display_idx = ((local_y - LAYERS_HEADER_H) / LAYER_ROW_H) as usize;
        if display_idx < self.shapes.len() {
            let shape_idx = self.shapes.len() - 1 - display_idx;
            self.selected = Some(shape_idx);
            self.tool = Tool::Select;
            return true;
        }
        false
    }

    fn handle_props_click(&mut self, pos: Point, area: Rect) -> bool {
        let panel_x = area.x0 + area.width() - PROPS_W;
        let base_y = area.y0 + props_palette_base_y();
        for i in 0..self.palette.len() {
            if props_palette_swatch_rect(i, panel_x, base_y).contains(pos) {
                self.set_palette_color(i);
                return true;
            }
        }
        false
    }
}

// ── Drawing helpers ─────────────────────────────────────────────────

fn draw_shape_to_scene(
    scene: &mut Scene,
    shape: &DrawShape,
    alpha: f32,
    xf: Affine,
    font: Option<&FontData>,
) {
    let local = if shape.rotation.abs() > 1e-4 {
        Affine::rotate_about(shape.rotation, shape.center())
    } else {
        Affine::IDENTITY
    };
    let transform = xf * local;
    let c = shape.color;
    let col = Color::new([
        c.components[0],
        c.components[1],
        c.components[2],
        c.components[3] * alpha,
    ]);

    match &shape.kind {
        ShapeKind::Rect => {
            let r = Rect::new(
                shape.origin.x,
                shape.origin.y,
                shape.origin.x + shape.size.0,
                shape.origin.y + shape.size.1,
            );
            scene.fill(Fill::NonZero, transform, col, None, &RoundedRect::from_rect(r, 2.0));
        }
        ShapeKind::Circle => {
            let circ = Circle::new(shape.origin, shape.size.0.abs());
            scene.fill(Fill::NonZero, transform, col, None, &circ);
        }
        ShapeKind::Line { end } => {
            scene.stroke(
                &Stroke::new(shape.stroke_width.max(2.0)),
                transform,
                col,
                None,
                &Line::new(shape.origin, *end),
            );
        }
        ShapeKind::Freehand { points } => {
            if points.len() >= 2 {
                let mut path = BezPath::new();
                path.move_to(points[0]);
                for p in &points[1..] {
                    path.line_to(*p);
                }
                scene.stroke(
                    &Stroke::new(shape.stroke_width.max(2.0)),
                    transform,
                    col,
                    None,
                    &path,
                );
            }
        }
        ShapeKind::Text { text, font_size } => {
            if let Some(f) = font {
                if !text.is_empty() {
                    let glyphs = layout_glyphs(f, text, *font_size);
                    let text_xf = transform * Affine::translate((shape.origin.x, shape.origin.y));
                    scene
                        .draw_glyphs(f)
                        .font_size(*font_size as f32)
                        .transform(text_xf)
                        .brush(&col)
                        .draw(Fill::NonZero, glyphs.into_iter());
                }
            }
        }
        ShapeKind::Artboard => {
            let r = Rect::new(
                shape.origin.x,
                shape.origin.y,
                shape.origin.x + shape.size.0,
                shape.origin.y + shape.size.1,
            );
            let bg_alpha = 0.04 * alpha;
            let bg = Color::new([c.components[0], c.components[1], c.components[2], bg_alpha]);
            scene.fill(Fill::NonZero, transform, bg, None, &r);
            scene.stroke(&Stroke::new(1.5), transform, col, None, &r);
            if let Some(f) = font {
                let label_xf = transform * Affine::translate((shape.origin.x, shape.origin.y - 4.0));
                scene
                    .draw_glyphs(f)
                    .font_size(11.0)
                    .transform(label_xf)
                    .brush(&col)
                    .draw(Fill::NonZero, layout_glyphs(f, &shape.name, 11.0).into_iter());
            }
        }
    }
}

fn draw_user_shapes(scene: &mut Scene, shapes: &[DrawShape], xf: Affine, font: Option<&FontData>) {
    for s in shapes {
        draw_shape_to_scene(scene, s, 1.0, xf, font);
    }
}

fn draw_selection_handles(scene: &mut Scene, shape: &DrawShape, xf: Affine, zoom: f64) {
    let b = shape.bounds();
    let pad = 2.0 / zoom;
    let outline = Rect::new(b.x0 - pad, b.y0 - pad, b.x1 + pad, b.y1 + pad);
    let local = if shape.rotation.abs() > 1e-4 {
        Affine::rotate_about(shape.rotation, shape.center())
    } else {
        Affine::IDENTITY
    };
    scene.stroke(&Stroke::new(1.5 / zoom), xf * local, SELECT_BLUE, None, &outline);

    let handles = corner_handles_of(shape);
    let r = HANDLE_R / zoom;
    for h in &handles {
        let dot = Circle::new(*h, r);
        scene.fill(Fill::NonZero, xf, HANDLE_FILL, None, &dot);
        scene.stroke(&Stroke::new(1.5 / zoom), xf, HANDLE_STROKE, None, &dot);
    }

    let midpoints = [
        Point::new((b.x0 + b.x1) / 2.0, b.y0),
        Point::new(b.x1, (b.y0 + b.y1) / 2.0),
        Point::new((b.x0 + b.x1) / 2.0, b.y1),
        Point::new(b.x0, (b.y0 + b.y1) / 2.0),
    ];
    let mid_r = (HANDLE_R - 1.0) / zoom;
    for mp in &midpoints {
        let mp_rot = if shape.rotation.abs() > 1e-4 {
            let c = shape.center();
            let (cos, sin) = (shape.rotation.cos(), shape.rotation.sin());
            let (dx, dy) = (mp.x - c.x, mp.y - c.y);
            Point::new(c.x + dx * cos - dy * sin, c.y + dx * sin + dy * cos)
        } else {
            *mp
        };
        let dot = Circle::new(mp_rot, mid_r);
        scene.fill(Fill::NonZero, xf, HANDLE_FILL, None, &dot);
        scene.stroke(&Stroke::new(1.0 / zoom), xf, HANDLE_STROKE, None, &dot);
    }
}

fn draw_preview(scene: &mut Scene, shape: &DrawShape, xf: Affine, font: Option<&FontData>) {
    draw_shape_to_scene(scene, shape, 0.5, xf, font);
}

fn make_preview(
    tool: Tool,
    drag_start: Option<Point>,
    mouse_canvas: Point,
    current_color: Color,
    freehand_buf: &[Point],
) -> Option<DrawShape> {
    let start = drag_start?;
    match tool {
        Tool::Rect => {
            let x0 = start.x.min(mouse_canvas.x);
            let y0 = start.y.min(mouse_canvas.y);
            let w = (mouse_canvas.x - start.x).abs();
            let h = (mouse_canvas.y - start.y).abs();
            Some(DrawShape {
                name: String::new(),
                kind: ShapeKind::Rect,
                origin: Point::new(x0, y0),
                size: (w, h),
                rotation: 0.0,
                color: current_color,
                stroke_width: 2.0,
            })
        }
        Tool::Circle => {
            let r = ((mouse_canvas.x - start.x).powi(2) + (mouse_canvas.y - start.y).powi(2)).sqrt();
            Some(DrawShape {
                name: String::new(),
                kind: ShapeKind::Circle,
                origin: start,
                size: (r, r),
                rotation: 0.0,
                color: current_color,
                stroke_width: 2.0,
            })
        }
        Tool::Line => Some(DrawShape {
            name: String::new(),
            kind: ShapeKind::Line { end: mouse_canvas },
            origin: start,
            size: (0.0, 0.0),
            rotation: 0.0,
            color: current_color,
            stroke_width: 3.0,
        }),
        Tool::Freehand => {
            if freehand_buf.len() >= 2 {
                Some(DrawShape {
                    name: String::new(),
                    kind: ShapeKind::Freehand { points: freehand_buf.to_vec() },
                    origin: Point::ZERO,
                    size: (0.0, 0.0),
                    rotation: 0.0,
                    color: current_color,
                    stroke_width: 3.0,
                })
            } else {
                None
            }
        }
        Tool::Artboard => {
            let x0 = start.x.min(mouse_canvas.x);
            let y0 = start.y.min(mouse_canvas.y);
            let w = (mouse_canvas.x - start.x).abs();
            let h = (mouse_canvas.y - start.y).abs();
            Some(DrawShape {
                name: String::new(),
                kind: ShapeKind::Artboard,
                origin: Point::new(x0, y0),
                size: (w, h),
                rotation: 0.0,
                color: current_color,
                stroke_width: 1.5,
            })
        }
        Tool::Select | Tool::Text => None,
    }
}

// ── Tool icons ──────────────────────────────────────────────────────

fn draw_tool_icon(scene: &mut Scene, tool: Tool, cx: f64, cy: f64, xf: Affine, active: bool) {
    let ic = if active { ICON_ACTIVE } else { ICON_COLOR };
    let s = &Stroke::new(1.5);
    match tool {
        Tool::Select => {
            let mut p = BezPath::new();
            p.move_to((cx - 5.0, cy - 8.0));
            p.line_to((cx - 5.0, cy + 6.0));
            p.line_to((cx - 1.0, cy + 3.0));
            p.line_to((cx + 3.0, cy + 8.0));
            p.line_to((cx + 6.0, cy + 6.0));
            p.line_to((cx + 2.0, cy + 1.0));
            p.line_to((cx + 6.0, cy - 2.0));
            p.close_path();
            scene.fill(Fill::NonZero, xf, ic, None, &p);
        }
        Tool::Rect => {
            let r = Rect::new(cx - 8.0, cy - 6.0, cx + 8.0, cy + 6.0);
            scene.stroke(s, xf, ic, None, &r);
        }
        Tool::Circle => {
            let c = Circle::new((cx, cy), 8.0);
            scene.stroke(s, xf, ic, None, &c);
        }
        Tool::Line => {
            scene.stroke(s, xf, ic, None, &Line::new((cx - 8.0, cy + 6.0), (cx + 8.0, cy - 6.0)));
        }
        Tool::Freehand => {
            let mut p = BezPath::new();
            p.move_to((cx - 8.0, cy));
            p.curve_to((cx - 4.0, cy - 6.0), (cx, cy + 6.0), (cx + 4.0, cy - 3.0));
            p.line_to((cx + 8.0, cy + 1.0));
            scene.stroke(s, xf, ic, None, &p);
        }
        Tool::Text => {
            scene.stroke(s, xf, ic, None, &Line::new((cx - 7.0, cy - 8.0), (cx + 7.0, cy - 8.0)));
            scene.stroke(s, xf, ic, None, &Line::new((cx, cy - 8.0), (cx, cy + 8.0)));
            scene.stroke(&Stroke::new(1.5), xf, ic, None, &Line::new((cx - 3.0, cy + 8.0), (cx + 3.0, cy + 8.0)));
        }
        Tool::Artboard => {
            let l = 4.0;
            scene.stroke(s, xf, ic, None, &Line::new((cx - 8.0, cy - 6.0), (cx - 8.0 + l, cy - 6.0)));
            scene.stroke(s, xf, ic, None, &Line::new((cx - 8.0, cy - 6.0), (cx - 8.0, cy - 6.0 + l)));
            scene.stroke(s, xf, ic, None, &Line::new((cx + 8.0, cy - 6.0), (cx + 8.0 - l, cy - 6.0)));
            scene.stroke(s, xf, ic, None, &Line::new((cx + 8.0, cy - 6.0), (cx + 8.0, cy - 6.0 + l)));
            scene.stroke(s, xf, ic, None, &Line::new((cx - 8.0, cy + 6.0), (cx - 8.0 + l, cy + 6.0)));
            scene.stroke(s, xf, ic, None, &Line::new((cx - 8.0, cy + 6.0), (cx - 8.0, cy + 6.0 - l)));
            scene.stroke(s, xf, ic, None, &Line::new((cx + 8.0, cy + 6.0), (cx + 8.0 - l, cy + 6.0)));
            scene.stroke(s, xf, ic, None, &Line::new((cx + 8.0, cy + 6.0), (cx + 8.0, cy + 6.0 - l)));
        }
    }
}

fn draw_layer_icon(scene: &mut Scene, kind: &ShapeKind, cx: f64, cy: f64, xf: Affine) {
    let s = &Stroke::new(1.2);
    let col = TEXT_SECONDARY;
    match kind {
        ShapeKind::Rect => {
            scene.stroke(s, xf, col, None, &Rect::new(cx - 5.0, cy - 4.0, cx + 5.0, cy + 4.0));
        }
        ShapeKind::Circle => {
            scene.stroke(s, xf, col, None, &Circle::new((cx, cy), 5.0));
        }
        ShapeKind::Line { .. } => {
            scene.stroke(s, xf, col, None, &Line::new((cx - 5.0, cy + 4.0), (cx + 5.0, cy - 4.0)));
        }
        ShapeKind::Freehand { .. } => {
            let mut p = BezPath::new();
            p.move_to((cx - 5.0, cy));
            p.curve_to((cx - 2.0, cy - 4.0), (cx + 2.0, cy + 4.0), (cx + 5.0, cy));
            scene.stroke(s, xf, col, None, &p);
        }
        ShapeKind::Text { .. } => {
            scene.stroke(s, xf, col, None, &Line::new((cx - 4.0, cy - 5.0), (cx + 4.0, cy - 5.0)));
            scene.stroke(s, xf, col, None, &Line::new((cx, cy - 5.0), (cx, cy + 5.0)));
        }
        ShapeKind::Artboard => {
            let l = 3.0;
            scene.stroke(s, xf, col, None, &Line::new((cx - 5.0, cy - 4.0), (cx - 5.0 + l, cy - 4.0)));
            scene.stroke(s, xf, col, None, &Line::new((cx - 5.0, cy - 4.0), (cx - 5.0, cy - 4.0 + l)));
            scene.stroke(s, xf, col, None, &Line::new((cx + 5.0, cy - 4.0), (cx + 5.0 - l, cy - 4.0)));
            scene.stroke(s, xf, col, None, &Line::new((cx + 5.0, cy - 4.0), (cx + 5.0, cy - 4.0 + l)));
            scene.stroke(s, xf, col, None, &Line::new((cx - 5.0, cy + 4.0), (cx - 5.0 + l, cy + 4.0)));
            scene.stroke(s, xf, col, None, &Line::new((cx - 5.0, cy + 4.0), (cx - 5.0, cy + 4.0 - l)));
            scene.stroke(s, xf, col, None, &Line::new((cx + 5.0, cy + 4.0), (cx + 5.0 - l, cy + 4.0)));
            scene.stroke(s, xf, col, None, &Line::new((cx + 5.0, cy + 4.0), (cx + 5.0, cy + 4.0 - l)));
        }
    }
}

// ── Panel drawing ───────────────────────────────────────────────────

fn draw_layers_panel(
    scene: &mut Scene,
    shapes: &[DrawShape],
    selected: Option<usize>,
    hovered_layer: Option<usize>,
    area: Rect,
    xf: Affine,
    font: Option<&FontData>,
) {
    let ax = area.x0;
    let ah = area.height();
    scene.fill(Fill::NonZero, xf, PANEL_BG, None, &Rect::new(ax, area.y0, ax + LAYERS_W, area.y0 + ah));
    scene.stroke(&Stroke::new(1.0), xf, PANEL_BORDER, None, &Line::new((ax + LAYERS_W - 0.5, area.y0), (ax + LAYERS_W - 0.5, area.y0 + ah)));
    scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, area.y0, ax + LAYERS_W, area.y0 + LAYERS_HEADER_H));
    if let Some(f) = font {
        draw_text(scene, f, "Layers", ax + 12.0, area.y0 + 26.0, 13.0, TEXT_PRIMARY, xf);
    }

    for (display_i, shape) in shapes.iter().rev().enumerate() {
        let shape_idx = shapes.len() - 1 - display_i;
        let row_y = area.y0 + LAYERS_HEADER_H + display_i as f64 * LAYER_ROW_H;
        let row = Rect::new(ax, row_y, ax + LAYERS_W, row_y + LAYER_ROW_H);

        if row.y0 > area.y0 + ah {
            break;
        }

        let is_selected = selected == Some(shape_idx);
        let is_hovered = hovered_layer == Some(shape_idx);
        if is_selected {
            scene.fill(Fill::NonZero, xf, LAYER_SELECTED, None, &row);
        } else if is_hovered {
            scene.fill(Fill::NonZero, xf, LAYER_HOVER, None, &row);
        }

        let icon_x = ax + 20.0;
        let icon_y = row_y + LAYER_ROW_H / 2.0;
        draw_layer_icon(scene, &shape.kind, icon_x, icon_y, xf);

        let swatch_x = ax + 38.0;
        let swatch_y = row_y + (LAYER_ROW_H - 12.0) / 2.0;
        scene.fill(
            Fill::NonZero,
            xf,
            shape.color,
            None,
            &RoundedRect::from_rect(Rect::new(swatch_x, swatch_y, swatch_x + 12.0, swatch_y + 12.0), 2.0),
        );

        if let Some(f) = font {
            let text_color = if is_selected { ICON_ACTIVE } else { TEXT_PRIMARY };
            draw_text(scene, f, &shape.name, ax + 56.0, row_y + 20.0, 11.0, text_color, xf);
        }
    }
}

fn draw_properties_panel(
    scene: &mut Scene,
    shape: Option<&DrawShape>,
    palette: &[Color],
    current_color: Color,
    area: Rect,
    xf: Affine,
    font: Option<&FontData>,
) {
    let panel_x = area.x0 + area.width() - PROPS_W;
    let win_h = area.height();

    scene.fill(Fill::NonZero, xf, PANEL_BG, None, &Rect::new(panel_x, area.y0, panel_x + PROPS_W, area.y0 + win_h));
    scene.stroke(&Stroke::new(1.0), xf, PANEL_BORDER, None, &Line::new((panel_x + 0.5, area.y0), (panel_x + 0.5, area.y0 + win_h)));
    scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(panel_x, area.y0, panel_x + PROPS_W, area.y0 + PROPS_HEADER_H));
    if let Some(f) = font {
        draw_text(scene, f, "Properties", panel_x + 12.0, area.y0 + 26.0, 13.0, TEXT_PRIMARY, xf);
    }

    let shape = match shape {
        Some(s) => s,
        None => return,
    };
    let f = match font {
        Some(f) => f,
        None => return,
    };

    let label_x = panel_x + 12.0;
    let value_x = panel_x + PROPS_LABEL_W + 12.0;
    let mut y = area.y0 + PROPS_HEADER_H + 12.0;

    draw_text(scene, f, "X", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    draw_text(scene, f, &format!("{:.1}", shape.origin.x), value_x, y + 16.0, 11.0, TEXT_VALUE, xf);
    y += PROPS_ROW_H;
    draw_text(scene, f, "Y", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    draw_text(scene, f, &format!("{:.1}", shape.origin.y), value_x, y + 16.0, 11.0, TEXT_VALUE, xf);
    y += PROPS_ROW_H;
    y += 8.0;

    let b = shape.bounds();
    let bw = b.x1 - b.x0;
    let bh = b.y1 - b.y0;
    draw_text(scene, f, "W", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    draw_text(scene, f, &format!("{:.1}", bw), value_x, y + 16.0, 11.0, TEXT_VALUE, xf);
    y += PROPS_ROW_H;
    draw_text(scene, f, "H", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    draw_text(scene, f, &format!("{:.1}", bh), value_x, y + 16.0, 11.0, TEXT_VALUE, xf);
    y += PROPS_ROW_H;
    y += 8.0;

    draw_text(scene, f, "Rot", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    draw_text(scene, f, &format!("{:.1}\u{00b0}", shape.rotation.to_degrees()), value_x, y + 16.0, 11.0, TEXT_VALUE, xf);
    y += PROPS_ROW_H;
    y += 8.0;

    draw_text(scene, f, "Color", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    let sw = Rect::new(value_x, y + 4.0, value_x + 20.0, y + 24.0);
    scene.fill(Fill::NonZero, xf, shape.color, None, &RoundedRect::from_rect(sw, 3.0));
    y += PROPS_ROW_H;

    draw_text(scene, f, "Stroke", label_x, y + 16.0, 11.0, TEXT_SECONDARY, xf);
    draw_text(scene, f, &format!("{:.1}", shape.stroke_width), value_x, y + 16.0, 11.0, TEXT_VALUE, xf);
    y += PROPS_ROW_H;
    y += 12.0;

    for (i, col) in palette.iter().enumerate() {
        let sr = props_palette_swatch_rect(i, panel_x, y);
        scene.fill(Fill::NonZero, xf, *col, None, &RoundedRect::from_rect(sr, 3.0));
        if col.components == current_color.components {
            scene.stroke(&Stroke::new(2.0), xf, HANDLE_FILL, None, &RoundedRect::from_rect(sr.inflate(1.5, 1.5), 4.0));
        }
    }
}

fn draw_tool_dock(
    scene: &mut Scene,
    tool: Tool,
    current_color: Color,
    palette: &[Color],
    selected: Option<usize>,
    area: Rect,
    xf: Affine,
) {
    let has_del = selected.is_some();
    let dr = dock_rect(area, has_del);

    let shadow = Rect::new(dr.x0 - 1.0, dr.y0 + 2.0, dr.x1 + 1.0, dr.y1 + 4.0);
    scene.fill(Fill::NonZero, xf, DOCK_SHADOW, None, &RoundedRect::from_rect(shadow, DOCK_RADIUS));
    scene.fill(Fill::NonZero, xf, DOCK_BG, None, &RoundedRect::from_rect(dr, DOCK_RADIUS));

    for (i, t) in TOOLS.iter().enumerate() {
        let r = dock_tool_btn_rect(i, area, has_del);
        let active = *t == tool;
        let bg = if active { BTN_ACTIVE } else { BTN_IDLE };
        scene.fill(Fill::NonZero, xf, bg, None, &RoundedRect::from_rect(r, 6.0));
        let c = r.center();
        draw_tool_icon(scene, *t, c.x, c.y, xf, active);
    }

    let tools_end_x = dock_tool_btn_rect(TOOLS.len() - 1, area, has_del).x1 + DOCK_BTN_GAP;
    let div_x = tools_end_x + (DOCK_DIVIDER_W - DOCK_BTN_GAP) / 2.0;
    let div_top = dr.y0 + 10.0;
    let div_bottom = dr.y1 - 10.0;
    scene.stroke(&Stroke::new(1.0), xf, PANEL_BORDER, None, &Line::new((div_x, div_top), (div_x, div_bottom)));

    for (i, col) in palette.iter().enumerate() {
        let r = dock_swatch_rect(i, area, has_del);
        scene.fill(Fill::NonZero, xf, *col, None, &RoundedRect::from_rect(r, 4.0));
        if col.components == current_color.components {
            scene.stroke(&Stroke::new(2.0), xf, HANDLE_FILL, None, &RoundedRect::from_rect(r.inflate(1.5, 1.5), 5.0));
        }
    }

    if has_del {
        let swatch_end_x = dock_swatch_rect(palette.len() - 1, area, true).x1;
        let div2_x = swatch_end_x + DOCK_DIVIDER_W / 2.0;
        scene.stroke(&Stroke::new(1.0), xf, PANEL_BORDER, None, &Line::new((div2_x, div_top), (div2_x, div_bottom)));

        let del_r = dock_delete_rect(area);
        scene.fill(Fill::NonZero, xf, BTN_IDLE, None, &RoundedRect::from_rect(del_r, 6.0));
        let del_col = Color::new([0.9, 0.3, 0.3, 1.0]);
        let m = 9.0;
        scene.stroke(&Stroke::new(2.0), xf, del_col, None, &Line::new((del_r.x0 + m, del_r.y0 + m), (del_r.x1 - m, del_r.y1 - m)));
        scene.stroke(&Stroke::new(2.0), xf, del_col, None, &Line::new((del_r.x1 - m, del_r.y0 + m), (del_r.x0 + m, del_r.y1 - m)));
    }
}

fn draw_canvas_grid(scene: &mut Scene, xf: Affine, zoom: f64, pan: Vec2, canvas_area_w: f64, canvas_area_h: f64) {
    let spacing = GRID_SPACING;
    let dot_r = 1.0;
    let canvas_x0 = -pan.x / zoom;
    let canvas_y0 = -pan.y / zoom;
    let canvas_x1 = (canvas_area_w - pan.x) / zoom;
    let canvas_y1 = (canvas_area_h - pan.y) / zoom;
    let start_x = (canvas_x0 / spacing).floor() as i64;
    let start_y = (canvas_y0 / spacing).floor() as i64;
    let end_x = (canvas_x1 / spacing).ceil() as i64;
    let end_y = (canvas_y1 / spacing).ceil() as i64;
    let total = (end_x - start_x) * (end_y - start_y);
    if total > 5000 {
        return;
    }
    for gx in start_x..=end_x {
        for gy in start_y..=end_y {
            let cx = gx as f64 * spacing;
            let cy = gy as f64 * spacing;
            scene.fill(Fill::NonZero, xf, GRID_DOT, None, &Circle::new((cx, cy), dot_r));
        }
    }
}

fn build_demo_scene(scene: &mut Scene, t: f64, _frame: u64, xf: Affine) {
    let stroke_thin = Stroke::new(2.0);
    let stroke_med = Stroke::new(3.0);

    let rect = Rect::new(20.0, 80.0, 120.0, 150.0);
    scene.fill(Fill::NonZero, xf, Color::new([0.235, 0.510, 0.941, 1.0]), None, &rect);

    let rect2 = RoundedRect::new(140.0, 80.0, 240.0, 150.0, 0.0);
    scene.stroke(&stroke_med, xf, Color::new([0.941, 0.392, 0.235, 1.0]), None, &rect2);

    let circle = Circle::new((310.0, 115.0), 35.0);
    scene.fill(Fill::NonZero, xf, Color::new([0.392, 0.863, 0.392, 1.0]), None, &circle);

    let circle2 = Circle::new((400.0, 115.0), 35.0);
    scene.stroke(&stroke_med, xf, Color::new([0.863, 0.706, 0.196, 1.0]), None, &circle2);

    let rrect = RoundedRect::new(460.0, 80.0, 560.0, 150.0, 12.0);
    scene.fill(Fill::NonZero, xf, Color::new([0.706, 0.314, 0.784, 1.0]), None, &rrect);

    let ellipse = Ellipse::new((630.0, 115.0), (40.0, 30.0), 0.0);
    scene.fill(Fill::NonZero, xf, Color::new([0.0, 0.706, 0.706, 1.0]), None, &ellipse);

    let palette_colors: &[[f32; 3]] = &[
        [0.906, 0.298, 0.235], [0.902, 0.494, 0.133], [0.945, 0.769, 0.059], [0.180, 0.800, 0.443],
        [0.204, 0.596, 0.859], [0.608, 0.349, 0.714], [0.925, 0.941, 0.945], [0.204, 0.286, 0.369],
    ];
    for (i, rgb) in palette_colors.iter().enumerate() {
        let x = 20.0 + i as f64 * 70.0;
        let rect = Rect::new(x, 190.0, x + 60.0, 230.0);
        scene.fill(Fill::NonZero, xf, Color::new([rgb[0], rgb[1], rgb[2], 1.0]), None, &rect);
    }

    scene.stroke(&stroke_thin, xf, Color::new([0.392, 0.784, 1.0, 1.0]), None, &Line::new((20.0, 270.0), (180.0, 310.0)));
    scene.stroke(&stroke_thin, xf, Color::new([1.0, 0.392, 0.588, 1.0]), None, &Line::new((20.0, 310.0), (180.0, 270.0)));

    let anim_cx = 100.0 + (t * 2.0).sin() * 80.0;
    let anim_cy = 470.0 + (t * 3.0).cos() * 20.0;
    let bounce = Circle::new((anim_cx, anim_cy), 20.0);
    scene.fill(Fill::NonZero, xf, Color::new([1.0, 0.471, 0.314, 1.0]), None, &bounce);

    let rot_cx = 350.0;
    let rot_cy = 470.0;
    let sq_colors: &[[f32; 3]] = &[
        [0.906, 0.298, 0.235], [0.180, 0.800, 0.443], [0.204, 0.596, 0.859], [0.945, 0.769, 0.059],
    ];
    for (i, rgb) in sq_colors.iter().enumerate() {
        let angle = t + i as f64 * std::f64::consts::FRAC_PI_2;
        let sx = rot_cx + angle.cos() * 40.0;
        let sy = rot_cy + angle.sin() * 40.0;
        let sq = Rect::new(sx - 10.0, sy - 10.0, sx + 10.0, sy + 10.0);
        scene.fill(Fill::NonZero, xf, Color::new([rgb[0], rgb[1], rgb[2], 1.0]), None, &sq);
    }

    let pulse_r = 20.0 + (t * 4.0).sin() * 10.0;
    let pulse = Circle::new((550.0, 470.0), pulse_r);
    scene.stroke(&stroke_med, xf, Color::new([0.706, 0.392, 1.0, 1.0]), None, &pulse);

    let star_cx = 650.0;
    let star_cy = 470.0;
    let mut star = BezPath::new();
    for i in 0..10 {
        let angle = -std::f64::consts::FRAC_PI_2 + i as f64 * std::f64::consts::PI / 5.0;
        let r = if i % 2 == 0 { 30.0 } else { 14.0 };
        let pt = (star_cx + angle.cos() * r, star_cy + angle.sin() * r);
        if i == 0 { star.move_to(pt); } else { star.line_to(pt); }
    }
    star.close_path();
    let star_rot = Affine::rotate_about(t * 1.5, Point::new(star_cx, star_cy));
    scene.fill(Fill::NonZero, xf * star_rot, Color::new([0.980, 0.800, 0.082, 1.0]), None, &star);

    let mut wave = BezPath::new();
    wave.move_to((20.0, 530.0));
    for i in 1..60 {
        let x = 20.0 + i as f64 * 10.0;
        let y = 530.0 + (i as f64 * 0.3 + t * 2.0).sin() * 20.0;
        wave.line_to((x, y));
    }
    scene.stroke(&stroke_thin, xf, Color::new([0.0, 0.800, 0.400, 1.0]), None, &wave);
}

// ── Space trait impl ────────────────────────────────────────────────

impl Space for DesignSpace {
    fn name(&self) -> &str {
        "Design"
    }

    fn draw(
        &mut self,
        scene: &mut Scene,
        xf: Affine,
        area: Rect,
        font: &FontData,
        _mono_font: &FontData,
    ) {
        self.frame += 1;
        let t = self.start.elapsed().as_secs_f64();
        let canvas_xf = xf
            * Affine::translate((area.x0 + LAYERS_W, area.y0))
            * Affine::translate(self.pan)
            * Affine::scale(self.zoom);

        // Canvas area background
        scene.fill(
            Fill::NonZero,
            xf,
            CANVAS_BG,
            None,
            &Rect::new(area.x0 + LAYERS_W, area.y0, area.x0 + area.width() - PROPS_W, area.y0 + area.height()),
        );

        // Grid dots
        draw_canvas_grid(
            scene,
            canvas_xf,
            self.zoom,
            self.pan,
            area.width() - LAYERS_W - PROPS_W,
            area.height(),
        );

        // Demo scene
        build_demo_scene(scene, t, self.frame, canvas_xf);

        // User shapes
        draw_user_shapes(scene, &self.shapes, canvas_xf, Some(font));

        // Selection handles
        if let Some(idx) = self.selected {
            if idx < self.shapes.len() {
                draw_selection_handles(scene, &self.shapes[idx], canvas_xf, self.zoom);
            }
        }

        // Text cursor blink
        if let Some(idx) = self.editing_text {
            if idx < self.shapes.len() {
                let blink = (t * 2.0).sin() > 0.0;
                if blink {
                    if let ShapeKind::Text { text, font_size } = &self.shapes[idx].kind {
                        let glyphs = layout_glyphs(font, text, *font_size);
                        let cursor_x = glyphs
                            .last()
                            .map_or(0.0, |g| g.x + *font_size as f32 * 0.55)
                            as f64;
                        let o = self.shapes[idx].origin;
                        scene.stroke(
                            &Stroke::new(1.5),
                            canvas_xf,
                            SELECT_BLUE,
                            None,
                            &Line::new(
                                (o.x + cursor_x, o.y - font_size * 0.8),
                                (o.x + cursor_x, o.y + font_size * 0.2),
                            ),
                        );
                    }
                }
            }
        }

        // Preview while dragging
        if self.is_dragging && self.tool != Tool::Select && self.tool != Tool::Text {
            let canvas = self.to_canvas(self.mouse_logical, area);
            if let Some(preview) = make_preview(
                self.tool,
                self.drag_start,
                canvas,
                self.current_color,
                &self.freehand_buf,
            ) {
                draw_preview(scene, &preview, canvas_xf, Some(font));
            }
        }

        // Layers panel
        draw_layers_panel(
            scene,
            &self.shapes,
            self.selected,
            self.hovered_layer,
            area,
            xf,
            Some(font),
        );

        // Properties panel
        let selected_shape = self.selected.and_then(|idx| self.shapes.get(idx));
        draw_properties_panel(
            scene,
            selected_shape,
            &self.palette,
            self.current_color,
            area,
            xf,
            Some(font),
        );

        // Tool dock
        draw_tool_dock(
            scene,
            self.tool,
            self.current_color,
            &self.palette,
            self.selected,
            area,
            xf,
        );
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        let logical = Point::new(x, y);
        self.mouse_logical = logical;

        // Right/Middle click → pan
        if button == MouseButton::Right || button == MouseButton::Middle {
            self.is_panning = true;
            self.pan_anchor = logical;
            self.pan_anchor_val = self.pan;
            return;
        }

        if button != MouseButton::Left {
            return;
        }

        let canvas = self.to_canvas(logical, area);

        // Dock check
        let has_del = self.selected.is_some();
        if dock_rect(area, has_del).contains(logical) {
            self.handle_dock_click(logical, area);
            return;
        }

        // Layers panel
        if logical.x < area.x0 + LAYERS_W {
            self.handle_layers_click(logical, area);
            return;
        }

        // Properties panel
        if logical.x > area.x0 + area.width() - PROPS_W {
            if self.selected.is_some() {
                self.handle_props_click(logical, area);
            }
            return;
        }

        // Canvas interaction
        self.drag_start = Some(canvas);
        self.is_dragging = true;

        if self.editing_text.is_some() {
            self.editing_text = None;
            self.tool = Tool::Select;
            return;
        }

        match self.tool {
            Tool::Select => {
                if let Some(c) = self.hit_handle(canvas) {
                    self.resize_corner = Some(c);
                } else if let Some(idx) = self.find_shape_at(canvas) {
                    self.selected = Some(idx);
                    self.moving = true;
                } else {
                    self.selected = None;
                    self.moving = false;
                    self.is_dragging = false;
                }
            }
            Tool::Freehand => {
                self.freehand_buf.clear();
                self.freehand_buf.push(canvas);
            }
            Tool::Text => {
                self.is_dragging = false;
                let kind = ShapeKind::Text { text: String::new(), font_size: DEFAULT_FONT_SIZE };
                let name = self.shape_counters.next_name(&kind);
                let idx = self.shapes.len();
                self.shapes.push(DrawShape {
                    name,
                    kind,
                    origin: canvas,
                    size: (0.0, 0.0),
                    rotation: 0.0,
                    color: self.current_color,
                    stroke_width: 0.0,
                });
                self.selected = Some(idx);
                self.editing_text = Some(idx);
            }
            _ => {}
        }
    }

    fn handle_mouse_release(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button == MouseButton::Right || button == MouseButton::Middle {
            self.is_panning = false;
            return;
        }

        if self.is_panning {
            self.is_panning = false;
            return;
        }

        let canvas = self.to_canvas(Point::new(x, y), area);

        if self.is_dragging {
            let start = self.drag_start.unwrap_or(canvas);
            match self.tool {
                Tool::Rect => {
                    let x0 = start.x.min(canvas.x);
                    let y0 = start.y.min(canvas.y);
                    let w = (canvas.x - start.x).abs();
                    let h = (canvas.y - start.y).abs();
                    if w > 2.0 && h > 2.0 {
                        let kind = ShapeKind::Rect;
                        let name = self.shape_counters.next_name(&kind);
                        self.shapes.push(DrawShape { name, kind, origin: Point::new(x0, y0), size: (w, h), rotation: 0.0, color: self.current_color, stroke_width: 2.0 });
                    }
                }
                Tool::Circle => {
                    let r = ((canvas.x - start.x).powi(2) + (canvas.y - start.y).powi(2)).sqrt();
                    if r > 2.0 {
                        let kind = ShapeKind::Circle;
                        let name = self.shape_counters.next_name(&kind);
                        self.shapes.push(DrawShape { name, kind, origin: start, size: (r, r), rotation: 0.0, color: self.current_color, stroke_width: 2.0 });
                    }
                }
                Tool::Line => {
                    let d = ((canvas.x - start.x).powi(2) + (canvas.y - start.y).powi(2)).sqrt();
                    if d > 2.0 {
                        let kind = ShapeKind::Line { end: canvas };
                        let name = self.shape_counters.next_name(&kind);
                        self.shapes.push(DrawShape { name, kind, origin: start, size: (0.0, 0.0), rotation: 0.0, color: self.current_color, stroke_width: 3.0 });
                    }
                }
                Tool::Freehand => {
                    if self.freehand_buf.len() >= 2 {
                        let pts = std::mem::take(&mut self.freehand_buf);
                        let kind = ShapeKind::Freehand { points: pts };
                        let name = self.shape_counters.next_name(&kind);
                        self.shapes.push(DrawShape { name, kind, origin: Point::ZERO, size: (0.0, 0.0), rotation: 0.0, color: self.current_color, stroke_width: 3.0 });
                    }
                }
                Tool::Artboard => {
                    let x0 = start.x.min(canvas.x);
                    let y0 = start.y.min(canvas.y);
                    let w = (canvas.x - start.x).abs();
                    let h = (canvas.y - start.y).abs();
                    if w > 2.0 && h > 2.0 {
                        let kind = ShapeKind::Artboard;
                        let name = self.shape_counters.next_name(&kind);
                        self.shapes.push(DrawShape { name, kind, origin: Point::new(x0, y0), size: (w, h), rotation: 0.0, color: self.current_color, stroke_width: 1.5 });
                    }
                }
                Tool::Select => {
                    self.moving = false;
                    self.resize_corner = None;
                }
                Tool::Text => {}
            }
        }
        self.is_dragging = false;
        self.drag_start = None;
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let logical = Point::new(x, y);
        self.mouse_logical = logical;

        // Track hovered layer
        if logical.x < area.x0 + LAYERS_W && logical.y > area.y0 + LAYERS_HEADER_H {
            let display_idx = ((logical.y - area.y0 - LAYERS_HEADER_H) / LAYER_ROW_H) as usize;
            if display_idx < self.shapes.len() {
                self.hovered_layer = Some(self.shapes.len() - 1 - display_idx);
            } else {
                self.hovered_layer = None;
            }
        } else {
            self.hovered_layer = None;
        }

        if self.is_panning {
            self.pan = Vec2::new(
                self.pan_anchor_val.x + (logical.x - self.pan_anchor.x),
                self.pan_anchor_val.y + (logical.y - self.pan_anchor.y),
            );
            return true;
        }

        let canvas = self.to_canvas(logical, area);

        if self.is_dragging {
            if self.tool == Tool::Freehand {
                self.freehand_buf.push(canvas);
            } else if self.tool == Tool::Select {
                if let Some(handle) = self.resize_corner {
                    if let Some(idx) = self.selected {
                        if idx < self.shapes.len() {
                            let shape = &mut self.shapes[idx];
                            match &mut shape.kind {
                                ShapeKind::Rect | ShapeKind::Artboard => {
                                    let (ox, oy) = (shape.origin.x, shape.origin.y);
                                    let (w, h) = shape.size;
                                    if handle < 4 {
                                        let anchors = [
                                            Point::new(ox + w, oy + h),
                                            Point::new(ox, oy + h),
                                            Point::new(ox, oy),
                                            Point::new(ox + w, oy),
                                        ];
                                        let anchor = anchors[handle];
                                        let x0 = anchor.x.min(canvas.x);
                                        let y0 = anchor.y.min(canvas.y);
                                        let x1 = anchor.x.max(canvas.x);
                                        let y1 = anchor.y.max(canvas.y);
                                        shape.origin = Point::new(x0, y0);
                                        shape.size = (x1 - x0, y1 - y0);
                                    } else {
                                        match handle {
                                            4 => { let bottom = oy + h; let new_y = canvas.y.min(bottom); shape.origin.y = new_y; shape.size.1 = bottom - new_y; }
                                            5 => { shape.size.0 = (canvas.x - ox).max(1.0); }
                                            6 => { shape.size.1 = (canvas.y - oy).max(1.0); }
                                            7 => { let right = ox + w; let new_x = canvas.x.min(right); shape.origin.x = new_x; shape.size.0 = right - new_x; }
                                            _ => {}
                                        }
                                    }
                                }
                                ShapeKind::Circle => {
                                    let d = ((canvas.x - shape.origin.x).powi(2) + (canvas.y - shape.origin.y).powi(2)).sqrt();
                                    shape.size = (d, d);
                                }
                                ShapeKind::Line { end } => {
                                    if handle == 0 || handle == 1 || handle == 3 || handle == 7 || handle == 4 {
                                        shape.origin = canvas;
                                    } else {
                                        *end = canvas;
                                    }
                                }
                                ShapeKind::Freehand { points } => {
                                    let b = {
                                        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                                        for p in points.iter() { x0 = x0.min(p.x); y0 = y0.min(p.y); x1 = x1.max(p.x); y1 = y1.max(p.y); }
                                        (x0, y0, x1, y1)
                                    };
                                    let (bw, bh) = (b.2 - b.0, b.3 - b.1);
                                    if bw > 1.0 && bh > 1.0 {
                                        let (nx0, ny0, nx1, ny1) = if handle < 4 {
                                            let anchors = [(b.2, b.3), (b.0, b.3), (b.0, b.1), (b.2, b.1)];
                                            let a = anchors[handle];
                                            (a.0.min(canvas.x), a.1.min(canvas.y), a.0.max(canvas.x), a.1.max(canvas.y))
                                        } else {
                                            match handle {
                                                4 => (b.0, canvas.y.min(b.3), b.2, b.3),
                                                5 => (b.0, b.1, canvas.x.max(b.0 + 1.0), b.3),
                                                6 => (b.0, b.1, b.2, canvas.y.max(b.1 + 1.0)),
                                                7 => (canvas.x.min(b.2), b.1, b.2, b.3),
                                                _ => (b.0, b.1, b.2, b.3),
                                            }
                                        };
                                        let (nw, nh) = (nx1 - nx0, ny1 - ny0);
                                        if nw > 1.0 && nh > 1.0 {
                                            let sx = nw / bw;
                                            let sy = nh / bh;
                                            for p in points.iter_mut() {
                                                p.x = nx0 + (p.x - b.0) * sx;
                                                p.y = ny0 + (p.y - b.1) * sy;
                                            }
                                        }
                                    }
                                }
                                ShapeKind::Text { font_size, .. } => {
                                    let (ox, oy) = (shape.origin.x, shape.origin.y);
                                    let (w, h) = shape.size;
                                    if handle < 4 {
                                        let anchors = [Point::new(ox + w, oy + h), Point::new(ox, oy + h), Point::new(ox, oy), Point::new(ox + w, oy)];
                                        let anchor = anchors[handle];
                                        let x0 = anchor.x.min(canvas.x);
                                        let y0 = anchor.y.min(canvas.y);
                                        let x1 = anchor.x.max(canvas.x);
                                        let y1 = anchor.y.max(canvas.y);
                                        let new_h = y1 - y0;
                                        if h > 1.0 && new_h > 5.0 { *font_size *= new_h / h; }
                                        shape.origin = Point::new(x0, y0);
                                        shape.size = (x1 - x0, new_h);
                                    } else {
                                        match handle {
                                            4 => { let bottom = oy + h; let new_y = canvas.y.min(bottom); let new_h = bottom - new_y; if h > 1.0 && new_h > 5.0 { *font_size *= new_h / h; } shape.origin.y = new_y; shape.size.1 = new_h; }
                                            5 => { shape.size.0 = (canvas.x - ox).max(1.0); }
                                            6 => { let new_h = (canvas.y - oy).max(5.0); if h > 1.0 { *font_size *= new_h / h; } shape.size.1 = new_h; }
                                            7 => { let right = ox + w; let new_x = canvas.x.min(right); shape.origin.x = new_x; shape.size.0 = right - new_x; }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if self.moving {
                    if let Some(start) = self.drag_start {
                        let dx = canvas.x - start.x;
                        let dy = canvas.y - start.y;
                        if let Some(idx) = self.selected {
                            if idx < self.shapes.len() {
                                self.shapes[idx].translate(Vec2::new(dx, dy));
                            }
                        }
                        self.drag_start = Some(canvas);
                    }
                }
            }
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, dx: f64, dy: f64, mouse_x: f64, mouse_y: f64, area: Rect) {
        let logical = Point::new(mouse_x, mouse_y);
        // Only canvas area
        if logical.x < area.x0 + LAYERS_W || logical.x > area.x0 + area.width() - PROPS_W {
            return;
        }
        // dy is already raw delta
        // Check for zoom (Cmd/Ctrl held is handled by main.rs passing modifiers)
        // For design space, scroll = rotate selected or pan; zoom handled via key
        if let Some(idx) = self.selected {
            if idx < self.shapes.len() {
                self.shapes[idx].rotation += dy * 0.05;
            }
        } else {
            self.pan.x += dx;
            self.pan.y += dy;
        }
    }

    fn handle_key(&mut self, key: &Key, modifiers: ModifiersState) -> bool {
        let cmd = modifiers.super_key() || modifiers.control_key();

        // Zoom with Cmd +/-
        if cmd {
            match key {
                Key::Character(c) => match c.as_str() {
                    "=" | "+" => { self.zoom_at(self.mouse_logical, 1.25, Rect::ZERO); return true; }
                    "-" | "_" => { self.zoom_at(self.mouse_logical, 0.8, Rect::ZERO); return true; }
                    "0" => { self.zoom = 1.0; self.pan = Vec2::ZERO; return true; }
                    _ => {}
                },
                _ => {}
            }
        }

        // Text editing mode
        if let Some(idx) = self.editing_text {
            if idx < self.shapes.len() {
                match key {
                    Key::Named(NamedKey::Escape | NamedKey::Enter) => {
                        self.editing_text = None;
                        self.tool = Tool::Select;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        if let ShapeKind::Text { text, .. } = &mut self.shapes[idx].kind {
                            text.pop();
                        }
                    }
                    _ => return false, // let handle_char deal with it
                }
            } else {
                self.editing_text = None;
            }
            return true;
        }

        // Normal shortcuts
        match key {
            Key::Character(c) => match c.as_str() {
                "s" | "S" | "v" | "V" => self.tool = Tool::Select,
                "r" | "R" => self.tool = Tool::Rect,
                "c" | "C" => self.tool = Tool::Circle,
                "l" | "L" => self.tool = Tool::Line,
                "f" | "F" | "p" | "P" => self.tool = Tool::Freehand,
                "t" | "T" => self.tool = Tool::Text,
                "a" | "A" => self.tool = Tool::Artboard,
                "1" => self.set_palette_color(0),
                "2" => self.set_palette_color(1),
                "3" => self.set_palette_color(2),
                "4" => self.set_palette_color(3),
                "5" => self.set_palette_color(4),
                "6" => self.set_palette_color(5),
                "7" => self.set_palette_color(6),
                "8" => self.set_palette_color(7),
                "=" | "+" => { self.zoom_at(self.mouse_logical, 1.25, Rect::ZERO); }
                "-" | "_" => { self.zoom_at(self.mouse_logical, 0.8, Rect::ZERO); }
                "0" => { self.zoom = 1.0; self.pan = Vec2::ZERO; }
                _ => return false,
            },
            Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                if let Some(idx) = self.selected.take() {
                    if idx < self.shapes.len() {
                        self.shapes.remove(idx);
                    }
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.editing_text = None;
                self.selected = None;
                self.tool = Tool::Select;
            }
            _ => return false,
        }
        true
    }

    fn handle_char(&mut self, ch: &str, _modifiers: ModifiersState) -> bool {
        if let Some(idx) = self.editing_text {
            if idx < self.shapes.len() {
                if let ShapeKind::Text { text, .. } = &mut self.shapes[idx].kind {
                    text.push_str(ch);
                    return true;
                }
            }
        }
        false
    }
}
