use crate::component::{
    Background, ChildrenInfo, Component, Id, ShouldHaltPropagation, SizingInfo,
};
use crate::{color, draw, event as e, layout};
use layout::{LiteralPlacer, Placer, Sz};

use std::sync::Mutex;

use std::cell::UnsafeCell;

pub struct DisplayTextBox {
    text: String,
    inf: SizingInfo,
    sz: UnsafeCell<Sz>,
    placer: Box<Placer>,
    cbs: Vec<e::CBFn>,
    emp_vec: Vec<Id>,
    parent: Id,
    has_resized: Mutex<bool>,
}
impl DisplayTextBox {
    fn new(text: Option<String>, parent: Id) -> Self {
        Self {
            text: text.unwrap_or_else(|| String::default()),
            inf: SizingInfo::default(),
            sz: UnsafeCell::new(Sz::new(0, 0)),
            placer: Box::new(layout::LiteralPlacer::new()),
            cbs: vec![],
            parent: parent,
            emp_vec: vec![],
            has_resized: Mutex::new(false),
        }
    }
}
impl Component for DisplayTextBox {
    fn sz_info(&self) -> Option<&SizingInfo> {
        Some(&self.inf)
    }
    fn children_info(&self) -> Option<&ChildrenInfo> {
        None
    }
    fn parent(&self) -> Option<&Id> {
        Some(&self.parent)
    }
    fn bg(&self) -> Option<Background> {
        Some(Background::Color(color::TRANSPARENT.clone()))
    }
    // Changes dynamically
    fn set_placer(&self, p: &Box<Placer>) {}
    fn resize(&self, sz: Sz) {
        unsafe {
            *self.sz.get() = sz;
        }
        *self.has_resized.lock().unwrap() = true;
    }
    fn set_dirty(&mut self) {
        self.has_resized = Mutex::new(true);
    }
    fn assign_listener(&self) {
        // TODO figure out how this one works
    }
    // draw
    fn need_redraw(&self) -> bool {
        *self.has_resized.lock().unwrap()
    }
    fn draw(&self) -> Vec<draw::Cmd> {
        vec![]
    }
}
