#![allow(dead_code)]

use vello::kurbo::{Affine, Line, Point, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

use crate::text::draw_text;
use crate::theme::*;

use super::Space;

const HEADER_H: f64 = 48.0;
const SIDEBAR_W: f64 = 220.0;
const NODE_W: f64 = 160.0;
const NODE_H: f64 = 80.0;
const PORT_R: f64 = 5.0;
const FONT_SIZE: f64 = 12.0;
const TITLE_SIZE: f64 = 11.0;
const PAD: f64 = 12.0;
const ITEM_H: f64 = 32.0;

const ARCH_BG: Color = Color::new([0.10, 0.10, 0.13, 1.0]);
const ARCH_SIDEBAR: Color = Color::new([0.08, 0.08, 0.10, 1.0]);
const CANVAS_BG_ARCH: Color = Color::new([0.11, 0.11, 0.14, 1.0]);
const NODE_BG: Color = Color::new([0.16, 0.16, 0.20, 1.0]);
const NODE_HEADER: Color = Color::new([0.20, 0.24, 0.40, 1.0]);
const NODE_BORDER: Color = Color::new([0.28, 0.32, 0.50, 1.0]);
const NODE_SELECTED_BORDER: Color = Color::new([0.40, 0.55, 0.95, 1.0]);
const PORT_COLOR: Color = Color::new([0.50, 0.70, 0.95, 1.0]);
const WIRE_COLOR: Color = Color::new([0.40, 0.55, 0.75, 0.7]);
const ACCENT_ARCH: Color = Color::new([0.95, 0.60, 0.20, 1.0]);
const GRID_LINE: Color = Color::new([0.14, 0.14, 0.17, 1.0]);

#[derive(Clone)]
struct Node {
    title: String,
    subtitle: String,
    x: f64,
    y: f64,
    inputs: Vec<String>,
    outputs: Vec<String>,
}

struct Wire {
    from_node: usize,
    from_port: usize,
    to_node: usize,
    to_port: usize,
}

pub struct ArchitectSpace {
    nodes: Vec<Node>,
    wires: Vec<Wire>,
    selected_node: Option<usize>,
    dragging: Option<(usize, f64, f64)>, // node, offset_x, offset_y
    canvas_offset: (f64, f64),
    zoom: f64,
    hovered_node: Option<usize>,
    blueprints: Vec<String>,
    active_blueprint: usize,
}

impl ArchitectSpace {
    pub fn new() -> Self {
        Self {
            nodes: vec![
                Node {
                    title: "Frontend".into(), subtitle: "Vello + Winit".into(),
                    x: 100.0, y: 120.0,
                    inputs: vec![], outputs: vec!["events".into(), "scene".into()],
                },
                Node {
                    title: "Space Router".into(), subtitle: "Plugin loader".into(),
                    x: 360.0, y: 80.0,
                    inputs: vec!["events".into()], outputs: vec!["native".into(), "wasm".into()],
                },
                Node {
                    title: "Native Plugin".into(), subtitle: "dlopen + C ABI".into(),
                    x: 620.0, y: 40.0,
                    inputs: vec!["scene".into()], outputs: vec!["draw".into()],
                },
                Node {
                    title: "WASM Plugin".into(), subtitle: "wasmtime".into(),
                    x: 620.0, y: 200.0,
                    inputs: vec!["host_fns".into()], outputs: vec!["draw".into()],
                },
                Node {
                    title: "Renderer".into(), subtitle: "wgpu surface".into(),
                    x: 880.0, y: 120.0,
                    inputs: vec!["scene".into()], outputs: vec![],
                },
            ],
            wires: vec![
                Wire { from_node: 0, from_port: 0, to_node: 1, to_port: 0 },
                Wire { from_node: 1, from_port: 0, to_node: 2, to_port: 0 },
                Wire { from_node: 1, from_port: 1, to_node: 3, to_port: 0 },
                Wire { from_node: 2, from_port: 0, to_node: 4, to_port: 0 },
                Wire { from_node: 3, from_port: 0, to_node: 4, to_port: 0 },
            ],
            selected_node: None,
            dragging: None,
            canvas_offset: (0.0, 0.0),
            zoom: 1.0,
            hovered_node: None,
            blueprints: vec!["Plugin Architecture".into(), "Data Flow".into(), "UI Components".into()],
            active_blueprint: 0,
        }
    }

    fn node_output_pos(&self, node: &Node, port: usize) -> Point {
        let port_y = node.y + 30.0 + port as f64 * 20.0;
        Point::new(node.x + NODE_W, port_y)
    }

    fn node_input_pos(&self, node: &Node, port: usize) -> Point {
        let port_y = node.y + 30.0 + port as f64 * 20.0;
        Point::new(node.x, port_y)
    }
}

impl Space for ArchitectSpace {
    fn name(&self) -> &str {
        "Architect"
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

        // Canvas background
        scene.fill(Fill::NonZero, xf, CANVAS_BG_ARCH, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Grid
        let grid_spacing = 40.0;
        let ox = self.canvas_offset.0 % grid_spacing;
        let oy = self.canvas_offset.1 % grid_spacing;
        let mut gx = ax + ox;
        while gx < ax + ww {
            scene.stroke(&Stroke::new(0.5), xf, GRID_LINE, None, &Line::new((gx, ay), (gx, ay + wh)));
            gx += grid_spacing;
        }
        let mut gy = ay + oy;
        while gy < ay + wh {
            scene.stroke(&Stroke::new(0.5), xf, GRID_LINE, None, &Line::new((ax, gy), (ax + ww, gy)));
            gy += grid_spacing;
        }

        // Sidebar
        scene.fill(Fill::NonZero, xf, ARCH_SIDEBAR, None, &Rect::new(ax, ay, ax + SIDEBAR_W, ay + wh));
        scene.fill(Fill::NonZero, xf, PANEL_HEADER_BG, None, &Rect::new(ax, ay, ax + SIDEBAR_W, ay + HEADER_H));
        draw_text(scene, font, "Blueprints", ax + PAD, ay + HEADER_H * 0.66, FONT_SIZE + 1.0, ACCENT_ARCH, xf);

        for (i, bp) in self.blueprints.iter().enumerate() {
            let y = ay + HEADER_H + i as f64 * ITEM_H;
            let bg = if i == self.active_blueprint { Color::new([0.14, 0.16, 0.24, 1.0]) } else { ARCH_SIDEBAR };
            scene.fill(Fill::NonZero, xf, bg, None, &Rect::new(ax, y, ax + SIDEBAR_W, y + ITEM_H));
            draw_text(scene, font, bp, ax + PAD, y + ITEM_H * 0.7, FONT_SIZE, TEXT_COLOR, xf);
        }

        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + SIDEBAR_W - 1.0, ay, ax + SIDEBAR_W, ay + wh));

        // Draw wires
        let canvas_x = ax + SIDEBAR_W + self.canvas_offset.0;
        let canvas_y = ay + self.canvas_offset.1;

        for wire in &self.wires {
            let from = &self.nodes[wire.from_node];
            let to = &self.nodes[wire.to_node];
            let p1 = self.node_output_pos(from, wire.from_port);
            let p2 = self.node_input_pos(to, wire.to_port);

            let start = Point::new(canvas_x + p1.x, canvas_y + p1.y);
            let end = Point::new(canvas_x + p2.x, canvas_y + p2.y);

            // Bezier curve for wire
            let mid_x = (start.x + end.x) / 2.0;
            let mut path = vello::kurbo::BezPath::new();
            path.move_to(start);
            path.curve_to((mid_x, start.y), (mid_x, end.y), (end.x, end.y));
            scene.stroke(&Stroke::new(2.0), xf, WIRE_COLOR, None, &path);
        }

        // Draw nodes
        for (i, node) in self.nodes.iter().enumerate() {
            let nx = canvas_x + node.x;
            let ny = canvas_y + node.y;
            let node_rect = Rect::new(nx, ny, nx + NODE_W, ny + NODE_H);

            // Node body
            scene.fill(Fill::NonZero, xf, NODE_BG, None, &RoundedRect::from_rect(node_rect, 8.0));

            // Node header
            let header_rect = Rect::new(nx, ny, nx + NODE_W, ny + 24.0);
            scene.fill(Fill::NonZero, xf, NODE_HEADER, None, &RoundedRect::from_rect(header_rect, 8.0));
            // Fix bottom corners of header
            scene.fill(Fill::NonZero, xf, NODE_HEADER, None, &Rect::new(nx, ny + 16.0, nx + NODE_W, ny + 24.0));

            // Border
            let border_color = if self.selected_node == Some(i) { NODE_SELECTED_BORDER } else { NODE_BORDER };
            scene.stroke(&Stroke::new(1.5), xf, border_color, None, &RoundedRect::from_rect(node_rect, 8.0));

            // Title
            draw_text(scene, font, &node.title, nx + 8.0, ny + 17.0, TITLE_SIZE, Color::new([1.0, 1.0, 1.0, 1.0]), xf);
            draw_text(scene, font, &node.subtitle, nx + 8.0, ny + 40.0, TITLE_SIZE - 1.0, TEXT_SECONDARY, xf);

            // Input ports
            for (pi, port_name) in node.inputs.iter().enumerate() {
                let py = ny + 30.0 + pi as f64 * 20.0;
                scene.fill(Fill::NonZero, xf, PORT_COLOR, None, &vello::kurbo::Circle::new((nx, py), PORT_R));
                draw_text(scene, font, port_name, nx + 8.0, py + 4.0, 9.0, TEXT_SECONDARY, xf);
            }

            // Output ports
            for (pi, port_name) in node.outputs.iter().enumerate() {
                let py = ny + 30.0 + pi as f64 * 20.0;
                scene.fill(Fill::NonZero, xf, PORT_COLOR, None, &vello::kurbo::Circle::new((nx + NODE_W, py), PORT_R));
                let pw = port_name.len() as f64 * 5.5;
                draw_text(scene, font, port_name, nx + NODE_W - pw - 8.0, py + 4.0, 9.0, TEXT_SECONDARY, xf);
            }
        }
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left { return; }
        let mx = x - area.x0;
        let my = y - area.y0;

        // Sidebar clicks
        if mx < SIDEBAR_W {
            if my > HEADER_H {
                let idx = ((my - HEADER_H) / ITEM_H) as usize;
                if idx < self.blueprints.len() {
                    self.active_blueprint = idx;
                }
            }
            return;
        }

        // Check node clicks
        let canvas_x = SIDEBAR_W + self.canvas_offset.0;
        let canvas_y = self.canvas_offset.1;

        for (i, node) in self.nodes.iter().enumerate() {
            let nx = canvas_x + node.x;
            let ny = canvas_y + node.y;
            if mx >= nx && mx <= nx + NODE_W && my >= ny && my <= ny + NODE_H {
                self.selected_node = Some(i);
                self.dragging = Some((i, mx - nx, my - ny));
                return;
            }
        }
        self.selected_node = None;
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {
        self.dragging = None;
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;

        if let Some((idx, ox, oy)) = self.dragging {
            let canvas_x = SIDEBAR_W + self.canvas_offset.0;
            let canvas_y = self.canvas_offset.1;
            self.nodes[idx].x = mx - canvas_x - ox;
            self.nodes[idx].y = my - canvas_y - oy;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, dx: f64, dy: f64, _mouse_x: f64, _mouse_y: f64, _area: Rect) {
        self.canvas_offset.0 += dx;
        self.canvas_offset.1 += dy;
    }

    fn handle_key(&mut self, _key: &Key, _modifiers: ModifiersState) -> bool {
        false
    }

    fn handle_char(&mut self, _ch: &str, _modifiers: ModifiersState) -> bool {
        false
    }
}
