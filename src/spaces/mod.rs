pub mod ai;
pub mod architect;
pub mod calendar;
pub mod chat;
pub mod code;
pub mod design;
pub mod docs;
pub mod git;
pub mod kanban;
pub mod notes;
pub mod paint;
pub mod releases;
pub mod terminal;

use vello::kurbo::{Affine, Rect};
use vello::peniko::FontData;
use vello::Scene;
use winit::event::MouseButton;
use winit::keyboard::{Key, ModifiersState};

pub trait Space {
    fn draw(
        &mut self,
        scene: &mut Scene,
        xf: Affine,
        area: Rect,
        font: &FontData,
        mono_font: &FontData,
    );
    fn handle_mouse_click(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
        area: Rect,
    );
    fn handle_mouse_release(
        &mut self,
        x: f64,
        y: f64,
        button: MouseButton,
        area: Rect,
    );
    fn handle_mouse_move(&mut self, x: f64, y: f64, area: Rect) -> bool;
    fn handle_scroll(
        &mut self,
        dx: f64,
        dy: f64,
        mouse_x: f64,
        mouse_y: f64,
        area: Rect,
    );
    fn handle_key(&mut self, key: &Key, modifiers: ModifiersState) -> bool;
    fn handle_char(&mut self, ch: &str, modifiers: ModifiersState) -> bool;
    fn name(&self) -> &str;
}

// Built-in spaces compiled into the binary
pub enum BuiltInSpace {
    Design(design::DesignSpace),
    Code(code::CodeSpace),
    Notes(notes::NotesSpace),
    Paint(paint::PaintSpace),
    Terminal(terminal::TerminalSpace),
    Calendar(calendar::CalendarSpace),
    Kanban(kanban::KanbanSpace),
    Chat(chat::ChatSpace),
    Ai(ai::AiSpace),
    Docs(docs::DocsSpace),
    Git(git::GitSpace),
    Architect(architect::ArchitectSpace),
    Releases(releases::ReleasesSpace),
}

// All loadable space variants
pub enum LoadedSpace {
    BuiltIn(BuiltInSpace),
    Native(crate::plugin::native::NativeSpace),
    Wasm(crate::plugin::wasm::WasmSpace),
}

impl LoadedSpace {
    pub fn as_space(&self) -> &dyn Space {
        match self {
            LoadedSpace::BuiltIn(b) => match b {
                BuiltInSpace::Design(s) => s,
                BuiltInSpace::Code(s) => s,
                BuiltInSpace::Notes(s) => s,
                BuiltInSpace::Paint(s) => s,
                BuiltInSpace::Terminal(s) => s,
                BuiltInSpace::Calendar(s) => s,
                BuiltInSpace::Kanban(s) => s,
                BuiltInSpace::Chat(s) => s,
                BuiltInSpace::Ai(s) => s,
                BuiltInSpace::Docs(s) => s,
                BuiltInSpace::Git(s) => s,
                BuiltInSpace::Architect(s) => s,
                BuiltInSpace::Releases(s) => s,
            },
            LoadedSpace::Native(s) => s,
            LoadedSpace::Wasm(s) => s,
        }
    }

    pub fn as_space_mut(&mut self) -> &mut dyn Space {
        match self {
            LoadedSpace::BuiltIn(b) => match b {
                BuiltInSpace::Design(s) => s,
                BuiltInSpace::Code(s) => s,
                BuiltInSpace::Notes(s) => s,
                BuiltInSpace::Paint(s) => s,
                BuiltInSpace::Terminal(s) => s,
                BuiltInSpace::Calendar(s) => s,
                BuiltInSpace::Kanban(s) => s,
                BuiltInSpace::Chat(s) => s,
                BuiltInSpace::Ai(s) => s,
                BuiltInSpace::Docs(s) => s,
                BuiltInSpace::Git(s) => s,
                BuiltInSpace::Architect(s) => s,
                BuiltInSpace::Releases(s) => s,
            },
            LoadedSpace::Native(s) => s,
            LoadedSpace::Wasm(s) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaded_space_builtin_design_name() {
        let design = design::DesignSpace::new();
        let mut space = LoadedSpace::BuiltIn(BuiltInSpace::Design(design));
        assert_eq!(space.as_space().name(), "Design");
        assert_eq!(space.as_space_mut().name(), "Design");
    }

    #[test]
    fn loaded_space_builtin_code_name() {
        let code = code::CodeSpace::new(None);
        let mut space = LoadedSpace::BuiltIn(BuiltInSpace::Code(code));
        assert_eq!(space.as_space().name(), "Code");
        assert_eq!(space.as_space_mut().name(), "Code");
    }

    #[test]
    fn loaded_space_builtin_design_handles_key_without_panic() {
        let design = design::DesignSpace::new();
        let mut space = LoadedSpace::BuiltIn(BuiltInSpace::Design(design));
        let key = Key::Character("a".into());
        // Should not panic
        let _ = space.as_space_mut().handle_key(&key, ModifiersState::empty());
    }
}
