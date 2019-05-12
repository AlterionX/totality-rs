use crate::{color, layout, event as e, draw};
use super::{Component, SizingInfo, Id, ShouldHaltPropagation};
use layout::{Sz, Placer, LiteralPlacer};
use draw::DrawCmd;

use std::cell::UnsafeCell;

pub struct DisplayTextBox {
    text: String,
    inf: SizingInfo,
    sz: UnsafeCell<Sz>,
    placer: Box<Placer>,
    cbs: Vec<e::CBFn>,
    emp_vec: Vec<Id>,
    parent: Id,
}
impl DisplayTextBox {
    fn new(text: Option<String>, parent: Id) -> Self {
        Self {
            text : text.unwrap_or_else(|| String::default()),
            inf: SizingInfo::default(),
            sz: UnsafeCell::new(Sz::new(0, 0)),
            placer: Box::new(layout::LiteralPlacer::new()),
            cbs: vec![],
            parent: parent,
            emp_vec: vec![],
        }
    }
}
impl Component for DisplayTextBox {
    fn min_sz(&self) -> Option<Sz> { self.inf.min }
    fn max_sz(&self) -> Option<Sz> { self.inf.max }
    fn preferred_sz(&self) -> Option<Sz> { self.inf.preferred }
    fn bg(&self) -> super::Background { super::Background::Color(color::TRANSPARENT.clone()) }
    fn placer(&self) -> &Box<Placer> {
        &self.placer
    }
    fn parent(&self) -> &Id { &self.parent }
    fn children(&self) -> &Vec<Id> { &self.emp_vec }
    // Changes dynamically
    fn set_placer(&self, p: &Box<Placer>) { }
    fn resize(&self, sz: Sz) { unsafe { *self.sz.get() = sz; } }
    fn fire_event(&mut self, e: &e::E) -> ShouldHaltPropagation {
        ShouldHaltPropagation(false)
    }
    fn assign_listener(&self) { // TODO figure out how this one works
    }
    // draw
    fn need_redraw(&self) -> bool { false }
    fn draw(&self) -> Vec<DrawCmd> { vec![] }
}
