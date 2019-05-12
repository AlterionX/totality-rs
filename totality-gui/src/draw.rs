use crate::layout::Sz;

pub enum DrawCmd {
    // TODO abstract out 2d draw commands a gui needs
    Text(String, Sz), Background(),
}

pub trait Drawer {
    fn draw(&self, cc: Vec<DrawCmd>);
}

