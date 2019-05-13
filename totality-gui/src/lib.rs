#![recursion_limit="512"]
#[macro_use]
extern crate lazy_static;
extern crate totality_model as geom;
// exports
pub mod color;
pub mod draw;
pub mod event;
pub mod components;
pub mod linkage;
pub mod layout;

use std::rc::Rc;

use linkage::*;
use draw::Drawer;
use layout::{Sz, Pos};
use components::{Component, Id};

pub struct Core<EL: EventLinkage, DL: DrawLinkage> {
    drawing_area: Sz,
    world_placement: geom::Model,
    cam: geom::camera::Camera,
    root: Id,
    drawer: Box<Drawer>,
    // indexed boxes for components
    pool: Vec<Rc<Component>>,
    pub elink: EL,
    pub dlink: DL,
}
impl <EL: EventLinkage, DL: DrawLinkage> Core<EL, DL> {
    fn new() {
    }
    fn launch(&self) {
        loop {
            // pull events
            // reinterpret events as gui actions
        }
        // TODO exit the gui
    }
    pub fn dispatch_draw(&self) {
    }
    fn reposition(&mut self, id: Id, p: &Pos) {
    }
    fn resize(&mut self, sz: Sz) {
        self.root.get().resize(sz);
    }
    fn draw(&self) {
        components::pre_iter(&self.root, &|c| self.drawer.draw(c.draw()));
    }
}

// Implementation
pub mod base_components;
