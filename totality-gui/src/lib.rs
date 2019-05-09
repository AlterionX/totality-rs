#![recursion_limit="512"]

#[macro_use]
extern crate lazy_static;

extern crate totality_model as geom;

pub mod color;
mod draw;
mod event;
mod components;
mod layout;
use event as e;
use color::Color;

use std::{cmp::{max, min}, rc::Rc};

use draw::{DrawCmd, Drawer};
use layout::{Sz, Pos, Placer};
use components::{Component, RootComponent, Id};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum StackOrder {
    HeadFirst,
    TailFirst,
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Img {
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Background {
    Color(Color),
    Img(Img),
    Stacked(Vec<Background>, StackOrder),
}

pub struct Core {
    drawing_area: Sz,
    world_placement: geom::Model,
    cam: geom::camera::Camera,
    root: Id,
    drawer: Box<Drawer>,
    // indexed boxes for components
    pool: Vec<Rc<Component>>,
}
impl Core {
    fn launch(&self) {
        loop {
            // pull events
            // reinterpret events as gui actions
        }
        // TODO exit the gui
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

pub struct Pane<P: Placer> {
    children: Vec<Box<Component>>,
    manager: P,
}

// impl <P: Placer> Component for Pane<P> {
//     fn sz(&self) -> Size {
//         Size(0, 0)
//     }
// }
// impl <P: Placer> RootComponent for Pane<P> {}

struct UI<T: RootComponent> {
    root: T
}
