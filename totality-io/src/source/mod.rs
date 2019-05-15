pub mod winit_convert;
pub use self::winit_convert as back;

use internal_events::hal as e;

#[derive(Debug, Copy, Clone)]
pub struct WindowSpecs {
    name: &'static str,
}
impl WindowSpecs {
    pub fn new(name: &'static str) -> WindowSpecs {
        WindowSpecs { name: name }
    }
}

pub trait IO {
    type Window;
    type Event;
    fn init(&mut self);
    fn next_events(&self, buf: &mut Vec<e::V>);
    fn create_window(&self, specs: WindowSpecs) -> Self::Window;
    fn to_v(e: Self::Event) -> e::V;
}
