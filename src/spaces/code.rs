#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Instant;

use streaming_iterator::StreamingIterator;
use vello::kurbo::{Affine, Rect, RoundedRect};
use vello::peniko::{Color, Fill, FontData};
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::text::{draw_text, measure_char_width};
use crate::theme::*;

use super::Space;

// Layout constants
const SIDEBAR_W: f64 = 200.0;
const GUTTER_W: f64 = 56.0;
const STATUS_H: f64 = 24.0;
const LINE_H: f64 = 20.0;
const FONT_SIZE: f64 = 13.0;
const SIDEBAR_FONT_SIZE: f64 = 12.0;
const STATUS_FONT_SIZE: f64 = 11.0;
const SIDEBAR_ITEM_H: f64 = 22.0;
const INDENT_W: f64 = 16.0;
const TEXT_PAD_LEFT: f64 = 8.0;

// ── Data Structures ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
    expanded: bool,
    depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HighlightKind {
    Keyword,
    String,
    Number,
    Comment,
    Function,
    Type,
    Macro,
    Attribute,
    Punctuation,
    Normal,
}

impl HighlightKind {
    fn color(self) -> Color {
        match self {
            Self::Keyword => KW_COLOR,
            Self::String => STRING_COLOR,
            Self::Number => NUMBER_COLOR,
            Self::Comment => COMMENT_COLOR,
            Self::Function => FN_COLOR,
            Self::Type => TYPE_COLOR,
            Self::Macro => MACRO_COLOR,
            Self::Attribute => ATTR_COLOR,
            Self::Punctuation => PUNCT_COLOR,
            Self::Normal => TEXT_COLOR,
        }
    }
}

#[derive(Debug, Clone)]
struct HighlightSpan {
    byte_start: usize,
    byte_end: usize,
    kind: HighlightKind,
}

#[derive(Debug, Clone)]
struct EditorBuffer {
    lines: Vec<String>,
    file_path: Option<PathBuf>,
    dirty: bool,
    line_highlights: Vec<Vec<HighlightSpan>>,
}

impl EditorBuffer {
    fn new() -> Self {
        Self {
            lines: vec![String::new()],
            file_path: None,
            dirty: false,
            line_highlights: vec![vec![]],
        }
    }

    fn from_file(path: &Path) -> Self {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content.lines().map(|l| l.to_string()).collect()
        };
        let len = lines.len();
        Self {
            lines,
            file_path: Some(path.to_path_buf()),
            dirty: false,
            line_highlights: vec![vec![]; len],
        }
    }

    fn full_text(&self) -> String {
        self.lines.join("\n")
    }

    fn save(&mut self) -> anyhow::Result<()> {
        if let Some(ref path) = self.file_path {
            let content = self.full_text();
            std::fs::write(path, &content)?;
            self.dirty = false;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Cursor {
    line: usize,
    col: usize,
}

impl Cursor {
    fn clamp(&mut self, buffer: &EditorBuffer) {
        if buffer.lines.is_empty() {
            self.line = 0;
            self.col = 0;
            return;
        }
        if self.line >= buffer.lines.len() {
            self.line = buffer.lines.len() - 1;
        }
        let line_len = buffer.lines[self.line].chars().count();
        if self.col > line_len {
            self.col = line_len;
        }
    }
}

// ── Tree-sitter highlight query ─────────────────────────────────────

const HIGHLIGHTS_QUERY: &str = r##"
; Comments
(line_comment) @comment
(block_comment) @comment

; Strings
(char_literal) @string
(string_literal) @string
(raw_string_literal) @string

; Numbers
(integer_literal) @number
(float_literal) @number

; Attributes
(attribute_item) @attribute
(inner_attribute_item) @attribute

; Macros
(macro_invocation macro: (identifier) @macro "!" @macro)

; Functions
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))
(call_expression function: (scoped_identifier "::" name: (identifier) @function))
(generic_function function: (identifier) @function)
(generic_function function: (field_expression field: (field_identifier) @function))
(function_item (identifier) @function)
(function_signature_item (identifier) @function)

; Types
(type_identifier) @type
(primitive_type) @type
((scoped_identifier path: (identifier) @type) (#match? @type "^[A-Z]"))
((scoped_type_identifier path: (identifier) @type) (#match? @type "^[A-Z]"))

; Keywords
"as" @keyword
"async" @keyword
"await" @keyword
"break" @keyword
"const" @keyword
"continue" @keyword
"default" @keyword
"dyn" @keyword
"else" @keyword
"enum" @keyword
"extern" @keyword
"fn" @keyword
"for" @keyword
"if" @keyword
"impl" @keyword
"in" @keyword
"let" @keyword
"loop" @keyword
"match" @keyword
"mod" @keyword
"move" @keyword
"pub" @keyword
"ref" @keyword
"return" @keyword
"static" @keyword
"struct" @keyword
"trait" @keyword
"type" @keyword
"unsafe" @keyword
"use" @keyword
"where" @keyword
"while" @keyword
"yield" @keyword
(crate) @keyword
(super) @keyword
(mutable_specifier) @keyword
(boolean_literal) @keyword

; Self
(self) @keyword

; Punctuation
"(" @punctuation
")" @punctuation
"[" @punctuation
"]" @punctuation
"{" @punctuation
"}" @punctuation
"::" @punctuation
":" @punctuation
"." @punctuation
"," @punctuation
";" @punctuation

; Identifiers (lowest priority)
(identifier) @normal
(field_identifier) @normal
"##;

// ── Helpers ─────────────────────────────────────────────────────────

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

// ── CodeSpace ───────────────────────────────────────────────────────

pub struct CodeSpace {
    // Editor
    buffer: EditorBuffer,
    cursor: Cursor,
    cursor_visible: bool,
    cursor_blink_time: Instant,
    scroll_y: f64,
    desired_col: usize,
    char_width: f64,

    // File explorer
    file_entries: Vec<FileEntry>,
    root_dir: PathBuf,
    sidebar_scroll_y: f64,
    hovered_entry: Option<usize>,

    // Tree-sitter
    ts_parser: tree_sitter::Parser,
    #[allow(dead_code)]
    ts_language: tree_sitter::Language,
    ts_tree: Option<tree_sitter::Tree>,
    ts_query: Option<tree_sitter::Query>,

    // Input state
    mouse_pos: (f64, f64),
}

impl CodeSpace {
    pub fn new(mono_font: Option<&FontData>) -> Self {
        let root_dir = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let root_dir = std::fs::canonicalize(&root_dir).unwrap_or(root_dir);

        let char_width = mono_font
            .map(|f| measure_char_width(f, FONT_SIZE))
            .unwrap_or(FONT_SIZE * 0.6);

        let ts_language = tree_sitter_rust::LANGUAGE.into();
        let mut ts_parser = tree_sitter::Parser::new();
        ts_parser.set_language(&ts_language).expect("Error setting tree-sitter language");

        let ts_query = match tree_sitter::Query::new(&ts_language, HIGHLIGHTS_QUERY) {
            Ok(q) => Some(q),
            Err(e) => {
                eprintln!("Warning: Could not compile highlight query: {e}");
                None
            }
        };

        let mut file_entries = Vec::new();
        Self::build_file_entries(&root_dir, 0, &mut file_entries);

        Self {
            buffer: EditorBuffer::new(),
            cursor: Cursor { line: 0, col: 0 },
            cursor_visible: true,
            cursor_blink_time: Instant::now(),
            scroll_y: 0.0,
            desired_col: 0,
            char_width,
            file_entries,
            root_dir,
            sidebar_scroll_y: 0.0,
            hovered_entry: None,
            ts_parser,
            ts_language,
            ts_tree: None,
            ts_query,
            mouse_pos: (0.0, 0.0),
        }
    }

    fn open_file(&mut self, path: &Path) {
        self.buffer = EditorBuffer::from_file(path);
        self.cursor = Cursor { line: 0, col: 0 };
        self.scroll_y = 0.0;
        self.run_highlighting();
    }

    fn run_highlighting(&mut self) {
        let source = self.buffer.full_text();
        let source_bytes = source.as_bytes();

        let tree = self.ts_parser.parse(&source, self.ts_tree.as_ref());
        self.ts_tree = tree;

        for spans in &mut self.buffer.line_highlights {
            spans.clear();
        }

        let Some(ref tree) = self.ts_tree else { return };
        let Some(ref query) = self.ts_query else { return };

        let mut query_cursor = tree_sitter::QueryCursor::new();
        let mut matches = query_cursor.matches(query, tree.root_node(), source_bytes);

        let mut all_spans: Vec<(usize, usize, HighlightKind)> = Vec::new();

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let name = &query.capture_names()[cap.index as usize];
                let kind = match name.as_ref() {
                    "keyword" => HighlightKind::Keyword,
                    "string" => HighlightKind::String,
                    "number" => HighlightKind::Number,
                    "comment" => HighlightKind::Comment,
                    "function" => HighlightKind::Function,
                    "type" => HighlightKind::Type,
                    "macro" => HighlightKind::Macro,
                    "attribute" => HighlightKind::Attribute,
                    "punctuation" => HighlightKind::Punctuation,
                    _ => continue,
                };
                let node = cap.node;
                all_spans.push((node.start_byte(), node.end_byte(), kind));
            }
        }

        let mut line_byte_offsets: Vec<usize> = Vec::with_capacity(self.buffer.lines.len());
        let mut offset = 0;
        for line in &self.buffer.lines {
            line_byte_offsets.push(offset);
            offset += line.len() + 1;
        }

        for (start, end, kind) in all_spans {
            let start_line = match line_byte_offsets.binary_search(&start) {
                Ok(i) => i,
                Err(i) => i.saturating_sub(1),
            };
            let end_line = match line_byte_offsets.binary_search(&end) {
                Ok(i) => if i > 0 && end == line_byte_offsets[i] { i - 1 } else { i },
                Err(i) => i.saturating_sub(1),
            };

            for line_idx in start_line..=end_line.min(self.buffer.lines.len() - 1) {
                let line_start = line_byte_offsets[line_idx];
                let line_end = line_start + self.buffer.lines[line_idx].len();
                let span_start_in_line = if start > line_start { start - line_start } else { 0 };
                let span_end_in_line = if end < line_end { end - line_start } else { self.buffer.lines[line_idx].len() };

                if span_start_in_line < span_end_in_line && line_idx < self.buffer.line_highlights.len() {
                    self.buffer.line_highlights[line_idx].push(HighlightSpan {
                        byte_start: span_start_in_line,
                        byte_end: span_end_in_line,
                        kind,
                    });
                }
            }
        }
    }

    fn insert_char(&mut self, ch: char) {
        self.cursor.clamp(&self.buffer);
        let line = &self.buffer.lines[self.cursor.line];
        let byte_idx = char_to_byte(line, self.cursor.col);
        let mut new_line = line.clone();
        new_line.insert(byte_idx, ch);
        self.buffer.lines[self.cursor.line] = new_line;
        self.cursor.col += 1;
        self.desired_col = self.cursor.col;
        self.buffer.dirty = true;
        self.run_highlighting();
    }

    fn insert_newline(&mut self) {
        self.cursor.clamp(&self.buffer);
        let line = self.buffer.lines[self.cursor.line].clone();
        let byte_idx = char_to_byte(&line, self.cursor.col);
        let before = line[..byte_idx].to_string();
        let after = line[byte_idx..].to_string();
        self.buffer.lines[self.cursor.line] = before;
        self.buffer.lines.insert(self.cursor.line + 1, after);
        self.buffer.line_highlights.insert(self.cursor.line + 1, vec![]);
        self.cursor.line += 1;
        self.cursor.col = 0;
        self.desired_col = 0;
        self.buffer.dirty = true;
        self.run_highlighting();
    }

    fn backspace(&mut self) {
        self.cursor.clamp(&self.buffer);
        if self.cursor.col > 0 {
            let line = &self.buffer.lines[self.cursor.line];
            let byte_idx = char_to_byte(line, self.cursor.col);
            let prev_byte_idx = char_to_byte(line, self.cursor.col - 1);
            let mut new_line = line.clone();
            new_line.replace_range(prev_byte_idx..byte_idx, "");
            self.buffer.lines[self.cursor.line] = new_line;
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            let current = self.buffer.lines.remove(self.cursor.line);
            self.buffer.line_highlights.remove(self.cursor.line);
            self.cursor.line -= 1;
            let prev_len = self.buffer.lines[self.cursor.line].chars().count();
            self.buffer.lines[self.cursor.line].push_str(&current);
            self.cursor.col = prev_len;
        }
        self.desired_col = self.cursor.col;
        self.buffer.dirty = true;
        self.run_highlighting();
    }

    fn delete(&mut self) {
        self.cursor.clamp(&self.buffer);
        let line_len = self.buffer.lines[self.cursor.line].chars().count();
        if self.cursor.col < line_len {
            let line = &self.buffer.lines[self.cursor.line];
            let byte_idx = char_to_byte(line, self.cursor.col);
            let next_byte_idx = char_to_byte(line, self.cursor.col + 1);
            let mut new_line = line.clone();
            new_line.replace_range(byte_idx..next_byte_idx, "");
            self.buffer.lines[self.cursor.line] = new_line;
        } else if self.cursor.line + 1 < self.buffer.lines.len() {
            let next = self.buffer.lines.remove(self.cursor.line + 1);
            self.buffer.line_highlights.remove(self.cursor.line + 1);
            self.buffer.lines[self.cursor.line].push_str(&next);
        }
        self.buffer.dirty = true;
        self.run_highlighting();
    }

    fn move_cursor_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.desired_col;
            self.cursor.clamp(&self.buffer);
        }
    }

    fn move_cursor_down(&mut self) {
        if self.cursor.line + 1 < self.buffer.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = self.desired_col;
            self.cursor.clamp(&self.buffer);
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.buffer.lines[self.cursor.line].chars().count();
        }
        self.desired_col = self.cursor.col;
    }

    fn move_cursor_right(&mut self) {
        let line_len = self.buffer.lines[self.cursor.line].chars().count();
        if self.cursor.col < line_len {
            self.cursor.col += 1;
        } else if self.cursor.line + 1 < self.buffer.lines.len() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
        self.desired_col = self.cursor.col;
    }

    fn ensure_cursor_visible(&mut self, area_h: f64) {
        let editor_h = area_h - STATUS_H;
        let cursor_y = self.cursor.line as f64 * LINE_H;
        if cursor_y < self.scroll_y {
            self.scroll_y = cursor_y;
        } else if cursor_y + LINE_H > self.scroll_y + editor_h {
            self.scroll_y = cursor_y + LINE_H - editor_h;
        }
        if self.scroll_y < 0.0 {
            self.scroll_y = 0.0;
        }
    }

    fn build_file_entries(dir: &Path, depth: usize, entries: &mut Vec<FileEntry>) {
        let Ok(read_dir) = std::fs::read_dir(dir) else { return };
        let mut items: Vec<(PathBuf, String, bool)> = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            let is_dir = path.is_dir();
            items.push((path, name, is_dir));
        }
        items.sort_by(|a, b| {
            b.2.cmp(&a.2).then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
        });
        for (path, name, is_dir) in items {
            entries.push(FileEntry { path, name, is_dir, expanded: false, depth });
        }
    }

    fn toggle_dir(&mut self, idx: usize) {
        if !self.file_entries[idx].is_dir {
            return;
        }
        let was_expanded = self.file_entries[idx].expanded;
        self.file_entries[idx].expanded = !was_expanded;

        if was_expanded {
            let depth = self.file_entries[idx].depth;
            let mut remove_count = 0;
            for i in (idx + 1)..self.file_entries.len() {
                if self.file_entries[i].depth > depth {
                    remove_count += 1;
                } else {
                    break;
                }
            }
            self.file_entries.drain((idx + 1)..(idx + 1 + remove_count));
        } else {
            let path = self.file_entries[idx].path.clone();
            let depth = self.file_entries[idx].depth;
            let mut children = Vec::new();
            Self::build_file_entries(&path, depth + 1, &mut children);
            let insert_pos = idx + 1;
            for (i, child) in children.into_iter().enumerate() {
                self.file_entries.insert(insert_pos + i, child);
            }
        }
    }
}

// ── Space trait impl ────────────────────────────────────────────────

impl Space for CodeSpace {
    fn name(&self) -> &str {
        "Code"
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

        // Update cursor blink
        let elapsed = self.cursor_blink_time.elapsed().as_secs_f64();
        self.cursor_visible = (elapsed * 2.0) as u64 % 2 == 0;

        // Background
        scene.fill(Fill::NonZero, xf, CODE_BG, None, &Rect::new(ax, ay, ax + ww, ay + wh));

        // Sidebar
        scene.fill(Fill::NonZero, xf, SIDEBAR_BG, None, &Rect::new(ax, ay, ax + SIDEBAR_W, ay + wh - STATUS_H));

        // Sidebar entries
        let sidebar_visible_start = (self.sidebar_scroll_y / SIDEBAR_ITEM_H) as usize;
        let sidebar_visible_count = ((wh - STATUS_H) / SIDEBAR_ITEM_H) as usize + 2;
        for i in sidebar_visible_start..self.file_entries.len().min(sidebar_visible_start + sidebar_visible_count) {
            let entry = &self.file_entries[i];
            let y = ay + i as f64 * SIDEBAR_ITEM_H - self.sidebar_scroll_y;
            if y + SIDEBAR_ITEM_H < ay || y > ay + wh - STATUS_H {
                continue;
            }

            if self.hovered_entry == Some(i) {
                scene.fill(Fill::NonZero, xf, SIDEBAR_HOVER, None, &Rect::new(ax, y, ax + SIDEBAR_W, y + SIDEBAR_ITEM_H));
            }

            let indent = entry.depth as f64 * INDENT_W + 8.0;
            let prefix = if entry.is_dir {
                if entry.expanded { "▾ " } else { "▸ " }
            } else {
                "  "
            };
            let label = format!("{}{}", prefix, entry.name);
            let color = if entry.is_dir { DIR_COLOR } else { SIDEBAR_TEXT };

            draw_text(scene, mono_font, &label, ax + indent, y + SIDEBAR_ITEM_H * 0.72, SIDEBAR_FONT_SIZE, color, xf);
        }

        // Sidebar separator
        scene.fill(Fill::NonZero, xf, SEPARATOR_COLOR, None, &Rect::new(ax + SIDEBAR_W - 1.0, ay, ax + SIDEBAR_W, ay + wh - STATUS_H));

        // Gutter
        let gutter_x = ax + SIDEBAR_W;
        scene.fill(Fill::NonZero, xf, GUTTER_BG, None, &Rect::new(gutter_x, ay, gutter_x + GUTTER_W, ay + wh - STATUS_H));

        // Editor area
        let editor_x = ax + SIDEBAR_W + GUTTER_W;
        let editor_h = wh - STATUS_H;
        let first_visible = (self.scroll_y / LINE_H) as usize;
        let visible_count = (editor_h / LINE_H) as usize + 2;

        for i in first_visible..self.buffer.lines.len().min(first_visible + visible_count) {
            let y = ay + i as f64 * LINE_H - self.scroll_y;
            if y + LINE_H < ay || y > ay + editor_h {
                continue;
            }

            // Active line highlight
            if i == self.cursor.line {
                scene.fill(Fill::NonZero, xf, ACTIVE_LINE_BG, None, &Rect::new(editor_x, y, ax + ww, y + LINE_H));
            }

            // Line number
            let line_num = format!("{:>4}", i + 1);
            let num_color = if i == self.cursor.line { TEXT_COLOR } else { LINE_NUM_COLOR };
            draw_text(scene, mono_font, &line_num, gutter_x + 4.0, y + LINE_H * 0.76, FONT_SIZE, num_color, xf);

            // Syntax-highlighted line content
            let line = &self.buffer.lines[i];
            if !line.is_empty() {
                let spans = &self.buffer.line_highlights[i];
                if spans.is_empty() {
                    draw_text(scene, mono_font, line, editor_x + TEXT_PAD_LEFT, y + LINE_H * 0.76, FONT_SIZE, TEXT_COLOR, xf);
                } else {
                    let line_bytes = line.as_bytes();
                    let mut byte_colors: Vec<HighlightKind> = vec![HighlightKind::Normal; line_bytes.len()];

                    for span in spans {
                        let start = span.byte_start.min(line_bytes.len());
                        let end = span.byte_end.min(line_bytes.len());
                        if span.kind != HighlightKind::Normal {
                            for bc in &mut byte_colors[start..end] {
                                *bc = span.kind;
                            }
                        }
                    }

                    let mut run_start_byte = 0;
                    let mut run_kind = byte_colors.first().copied().unwrap_or(HighlightKind::Normal);
                    let mut char_offset = 0usize;
                    let mut run_start_char = 0usize;

                    for (byte_idx, _ch) in line.char_indices() {
                        let kind = byte_colors[byte_idx];
                        if kind != run_kind {
                            let run_text = &line[run_start_byte..byte_idx];
                            if !run_text.is_empty() {
                                let x_pos = editor_x + TEXT_PAD_LEFT + run_start_char as f64 * self.char_width;
                                draw_text(scene, mono_font, run_text, x_pos, y + LINE_H * 0.76, FONT_SIZE, run_kind.color(), xf);
                            }
                            run_start_byte = byte_idx;
                            run_start_char = char_offset;
                            run_kind = kind;
                        }
                        char_offset += 1;
                    }
                    let run_text = &line[run_start_byte..];
                    if !run_text.is_empty() {
                        let x_pos = editor_x + TEXT_PAD_LEFT + run_start_char as f64 * self.char_width;
                        draw_text(scene, mono_font, run_text, x_pos, y + LINE_H * 0.76, FONT_SIZE, run_kind.color(), xf);
                    }
                }
            }
        }

        // Cursor
        if self.cursor_visible {
            let cursor_y = ay + self.cursor.line as f64 * LINE_H - self.scroll_y;
            let cursor_x = editor_x + TEXT_PAD_LEFT + self.cursor.col as f64 * self.char_width;
            if cursor_y >= ay && cursor_y < ay + editor_h {
                scene.fill(Fill::NonZero, xf, CURSOR_COLOR, None, &Rect::new(cursor_x, cursor_y + 2.0, cursor_x + 1.5, cursor_y + LINE_H - 2.0));
            }
        }

        // Scrollbar
        let total_h = self.buffer.lines.len() as f64 * LINE_H;
        if total_h > editor_h {
            let bar_h = (editor_h / total_h * editor_h).max(20.0);
            let bar_y = ay + self.scroll_y / total_h * editor_h;
            scene.fill(
                Fill::NonZero,
                xf,
                SCROLLBAR_COLOR,
                None,
                &RoundedRect::from_rect(Rect::new(ax + ww - 8.0, bar_y, ax + ww - 2.0, bar_y + bar_h), 3.0),
            );
        }

        // Status bar
        let status_y = ay + wh - STATUS_H;
        scene.fill(Fill::NonZero, xf, STATUS_BG, None, &Rect::new(ax, status_y, ax + ww, ay + wh));

        let filename = self.buffer.file_path.as_ref()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .unwrap_or_else(|| "untitled".to_string());
        let dirty_mark = if self.buffer.dirty { " [modified]" } else { "" };
        let status_left = format!("  {}{}", filename, dirty_mark);
        draw_text(scene, mono_font, &status_left, ax, status_y + STATUS_H * 0.72, STATUS_FONT_SIZE, STATUS_TEXT, xf);

        let lang = self.buffer.file_path.as_ref()
            .and_then(|p| p.extension())
            .map(|e| match e.to_str().unwrap_or("") {
                "rs" => "Rust",
                "toml" => "TOML",
                "md" => "Markdown",
                "js" | "mjs" => "JavaScript",
                "ts" => "TypeScript",
                "py" => "Python",
                "json" => "JSON",
                "yaml" | "yml" => "YAML",
                _ => "Plain Text",
            })
            .unwrap_or("Plain Text");
        let status_right = format!("Ln {}, Col {}    {}  ", self.cursor.line + 1, self.cursor.col + 1, lang);
        let right_w = status_right.len() as f64 * self.char_width * (STATUS_FONT_SIZE / FONT_SIZE);
        draw_text(scene, mono_font, &status_right, ax + ww - right_w - 8.0, status_y + STATUS_H * 0.72, STATUS_FONT_SIZE, STATUS_TEXT, xf);
    }

    fn handle_mouse_click(&mut self, x: f64, y: f64, button: MouseButton, area: Rect) {
        if button != MouseButton::Left {
            return;
        }

        let mx = x - area.x0;
        let my = y - area.y0;
        let wh = area.height();

        // Click in sidebar?
        if mx < SIDEBAR_W && my < wh - STATUS_H {
            let y_offset = my + self.sidebar_scroll_y;
            let idx = (y_offset / SIDEBAR_ITEM_H) as usize;
            if idx < self.file_entries.len() {
                if self.file_entries[idx].is_dir {
                    self.toggle_dir(idx);
                } else {
                    let path = self.file_entries[idx].path.clone();
                    self.open_file(&path);
                }
            }
        }
        // Click in editor area?
        else if mx >= SIDEBAR_W + GUTTER_W {
            let editor_x = mx - SIDEBAR_W - GUTTER_W - TEXT_PAD_LEFT;
            let editor_y = my + self.scroll_y;
            let line = (editor_y / LINE_H) as usize;
            let col = if editor_x > 0.0 {
                (editor_x / self.char_width).round() as usize
            } else {
                0
            };
            self.cursor.line = line;
            self.cursor.col = col;
            self.cursor.clamp(&self.buffer);
            self.desired_col = self.cursor.col;
            self.cursor_visible = true;
            self.cursor_blink_time = Instant::now();
        }
    }

    fn handle_mouse_release(&mut self, _x: f64, _y: f64, _button: MouseButton, _area: Rect) {}

    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool {
        let mx = x - area.x0;
        let my = y - area.y0;
        self.mouse_pos = (mx, my);
        let wh = area.height();

        if mx < SIDEBAR_W && my < wh - STATUS_H {
            let y_offset = my + self.sidebar_scroll_y;
            let idx = (y_offset / SIDEBAR_ITEM_H) as usize;
            let new_hover = if idx < self.file_entries.len() { Some(idx) } else { None };
            if new_hover != self.hovered_entry {
                self.hovered_entry = new_hover;
                return true;
            }
        } else if self.hovered_entry.is_some() {
            self.hovered_entry = None;
            return true;
        }
        false
    }

    fn handle_scroll(&mut self, _dx: f64, dy: f64, mouse_x: f64, _mouse_y: f64, area: Rect) {
        let mx = mouse_x - area.x0;
        let wh = area.height();

        if mx < SIDEBAR_W {
            self.sidebar_scroll_y = (self.sidebar_scroll_y - dy).max(0.0);
            let max_scroll = (self.file_entries.len() as f64 * SIDEBAR_ITEM_H - (wh - STATUS_H)).max(0.0);
            self.sidebar_scroll_y = self.sidebar_scroll_y.min(max_scroll);
        } else {
            self.scroll_y = (self.scroll_y - dy).max(0.0);
            let max_scroll = (self.buffer.lines.len() as f64 * LINE_H - (wh - STATUS_H)).max(0.0);
            self.scroll_y = self.scroll_y.min(max_scroll);
        }
    }

    fn handle_key(&mut self, key: &Key, modifiers: ModifiersState) -> bool {
        let cmd = modifiers.super_key() || modifiers.control_key();
        let area_h = 800.0; // fallback

        match key {
            Key::Named(NamedKey::ArrowUp) => {
                self.move_cursor_up();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.move_cursor_down();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.move_cursor_left();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.move_cursor_right();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::Home) => {
                self.cursor.col = 0;
                self.desired_col = 0;
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::End) => {
                self.cursor.clamp(&self.buffer);
                self.cursor.col = self.buffer.lines[self.cursor.line].chars().count();
                self.desired_col = self.cursor.col;
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::Backspace) => {
                self.backspace();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::Delete) => {
                self.delete();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::Enter) => {
                self.insert_newline();
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Named(NamedKey::Tab) => {
                for _ in 0..4 {
                    self.insert_char(' ');
                }
                self.ensure_cursor_visible(area_h);
                self.cursor_visible = true;
                self.cursor_blink_time = Instant::now();
            }
            Key::Character(ch) => {
                if cmd && ch.as_str() == "s" {
                    if let Err(e) = self.buffer.save() {
                        eprintln!("Save error: {e}");
                    }
                } else {
                    return false; // let handle_char deal with it
                }
            }
            _ => return false,
        }
        true
    }

    fn handle_char(&mut self, ch: &str, modifiers: ModifiersState) -> bool {
        let cmd = modifiers.super_key() || modifiers.control_key();
        if cmd {
            return false;
        }
        for c in ch.chars() {
            self.insert_char(c);
        }
        self.ensure_cursor_visible(800.0);
        self.cursor_visible = true;
        self.cursor_blink_time = Instant::now();
        true
    }
}
