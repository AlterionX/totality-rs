#![recursion_limit = "512"]
#[macro_use]
extern crate lazy_static;
extern crate log;
extern crate totality_model as geom;

// exports
pub mod color;
pub mod component;
pub mod draw;
pub mod event;
pub mod layout;
pub mod linkage;

use std::rc::Rc;

use component::{Component, Id};
use draw::Drawer;
use layout::{Pos, Sz};
use linkage::*;

#[allow(dead_code)]
use log::{debug, error, info, trace, warn};

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
impl<EL: EventLinkage, DL: DrawLinkage> Core<EL, DL> {
    fn new() {}
    fn launch(&self) {
        loop {
            // pull events
            // reinterpret events as gui actions
        }
        // TODO exit the gui
    }
    pub fn dispatch_draw(&self) {}
    fn reposition(&mut self, id: Id, p: &Pos) {}
    fn resize(&mut self, sz: Sz) {
        self.root.get().resize(sz);
    }
    fn draw(&self) {
        component::pre_iter(&self.root, &|c| self.drawer.draw(c.draw()));
    }
}

pub struct Manager {}
impl Manager {
    pub fn new() -> Self {
        Self {}
    }
    pub fn dispatch_draw(&self) {}
}
impl Drop for Manager {
    fn drop(&mut self) {
        info!("Shutting down gui systems.");
    }
}

// Sample implementations / reusable components
pub mod base_components;
