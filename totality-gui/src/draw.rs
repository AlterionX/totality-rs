use crate::{component::Background, layout::Cfg};

struct SVG {}
pub struct Placement {
    cfg: Cfg,
    stencil: SVG, // single, closed path svg
}
pub struct Span {}

pub struct Cmd(Placement, Content);
pub enum Content {
    // TODO abstract out 2d draw commands a gui needs
    Text(Span),
    Background(Background),
}

pub trait Drawer {
    fn draw(&self, cc: Vec<Cmd>);
}
