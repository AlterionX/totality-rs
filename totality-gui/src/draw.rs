use crate::layout::Sz;

pub enum DrawCmd<'a> {
    // TODO abstract out 2d draw commands a gui needs
    Text(&'a str, &'a Sz), Background(),
}

pub trait Drawer {
    fn draw(&self, cc: Vec<DrawCmd>);
}

