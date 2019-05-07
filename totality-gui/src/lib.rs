#![recursion_limit="512"]

#[macro_use]
extern crate lazy_static;

pub mod color;
pub mod event;
use event as e;
use color::Color;

use std::{cmp::{ max, min }, rc::Rc};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
struct Dim {
    pub hori: u64,
    pub vert: u64,
}
impl Dim {
    fn max(d0: &Self, d1: &Self) -> Self { Self {
        hori: max(d0.hori, d1.hori),
        vert: max(d0.vert, d0.vert),
    } }
    fn min(d0: &Self, d1: &Self) -> Self { Self {
        hori: min(d0.hori, d1.hori),
        vert: min(d0.vert, d0.vert),
    } }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Pos(Dim);
impl Pos {
    pub fn new(w: u64, h: u64) -> Pos { Pos(Dim { hori: w, vert: h }) }
    pub fn w(&self) -> &u64 { &self.0.hori }
    pub fn h(&self) -> &u64 { &self.0.vert }
}
impl From<Dim> for Pos {
    fn from(d: Dim) -> Self { Self(d) }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Sz(Dim);
impl Sz {
    pub fn new(x: u64, y: u64) -> Sz { Sz(Dim { hori: x, vert: y }) }
    pub fn x(&self) -> &u64 { &self.0.hori }
    pub fn y(&self) -> &u64 { &self.0.vert }
}
impl From<Dim> for Sz {
    fn from(d: Dim) -> Self { Self(d) }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Cfg(pub Pos, pub Sz);
impl Cfg {
    pub fn pos(&self) -> &Pos { let Cfg(ref p, _) = self; p }
    pub fn sz(&self) -> &Sz { let Cfg(_, ref sz) = self; sz }
}
impl From<(Pos, Sz)> for Cfg {
    fn from(tup: (Pos, Sz)) -> Self { Self(tup.0, tup.1) }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ShouldHaltPropagation(bool);
impl ShouldHaltPropagation {
    pub fn should(&self) -> &bool { &self.0 }
}
impl From<bool> for ShouldHaltPropagation {
    fn from(d: bool) -> Self { Self(d) }
}
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

pub enum DrawCmd {
    // TODO abstract out 2d draw commands a gui needs
}

pub trait Component {
    fn min_sz(&self) -> Sz;
    fn max_sz(&self) -> Sz;
    fn preferred_sz(&self) -> Sz;
    fn bg(&self) -> Background;
    fn configure(&self, cfg: Cfg);
    fn configuration(&self) -> Cfg;

    fn fire_event(&mut self, e: &e::E) -> ShouldHaltPropagation;

    fn get_parent(&self) -> &Box<Component>;
    fn get_children(&self) -> &Vec<Rc<Box<Component>>>;
    fn need_redraw(&self) -> bool;

    fn draw(&self) -> DrawCmd;
}
pub trait RootComponent : Component {}

pub trait Placer {
    fn need_redraw() -> bool;
    fn place(comp: &Vec<Box<Component>>) -> Vec<Cfg>;
}

pub trait Drawer {
    fn draw(&self, component: &Component);
}

pub struct Core {
    drawing_area: Sz,
    root: Rc<Box<RootComponent>>,
    drawer: Drawer,
}
impl Core {
    fn launch(&self) -> ! {
        loop {
        }
        // TODO exit the gui
    }
    fn resize(&mut self, create: Sz) {
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
