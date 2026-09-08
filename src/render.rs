use std::sync::Arc;

use vello::util::{RenderContext, RenderSurface};
use vello::{Renderer, RendererOptions};
use winit::dpi::LogicalSize;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

#[derive(Debug)]
pub enum RenderState {
    Active {
        surface: Box<RenderSurface<'static>>,
        valid_surface: bool,
        window: Arc<Window>,
    },
    Suspended(Option<Arc<Window>>),
}

pub fn create_window(event_loop: &ActiveEventLoop) -> Arc<Window> {
    let attr = Window::default_attributes()
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .with_resizable(true)
        .with_title("Construct");
    Arc::new(event_loop.create_window(attr).unwrap())
}

pub fn create_renderer(render_cx: &RenderContext, surface: &RenderSurface<'_>) -> Renderer {
    Renderer::new(
        &render_cx.devices[surface.dev_id].device,
        RendererOptions::default(),
    )
    .expect("Couldn't create renderer")
}
