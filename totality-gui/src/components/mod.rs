use std::rc::Rc;
use crate::Core;
use crate::layout::Sz;
use crate::event as e;
use crate::Background;
use crate::draw::DrawCmd;
use crate::layout::{Placer};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ShouldHaltPropagation(bool);
impl ShouldHaltPropagation {
    pub fn should(&self) -> &bool { &self.0 }
}
impl From<bool> for ShouldHaltPropagation {
    fn from(d: bool) -> Self { Self(d) }
}

pub struct Id(u64, Rc<Component>);
impl Id {
    pub fn get(&self) -> &Component { &*self.1 }
    pub fn get_id(&self) -> u64 { self.0 }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum IterMode {
    POST, PRE,
}
pub trait Component {
    // Mostly preset data
    fn min_sz(&self) -> Sz;
    fn max_sz(&self) -> Sz;
    fn preferred_sz(&self) -> Sz;
    fn id(&self) -> Id;
    fn bg(&self) -> Background;
    fn placer(&self) -> &Box<Placer>;
    fn parent(&self) -> &Id;
    fn children(&self) -> &Vec<Id>;
    // Changes dynamically
    fn set_placer(&self, p: &Box<Placer>);
    fn resize(&self, sz: Sz);
    fn fire_event(&mut self, e: &e::E) -> ShouldHaltPropagation;
    fn assign_listener(&self); // TODO figure out how this one works
    // draw
    fn need_redraw(&self) -> bool;
    fn draw(&self) -> Vec<DrawCmd>;
}

fn iter(root: &Id, mode: IterMode, f: &Fn(&Component)) {
    if mode == IterMode::PRE {
        f(root.get());
    }
    for child in root.get().children().iter() {
        pre_iter(child, f);
    }
    if mode == IterMode::POST {
        f(root.get());
    }
}
pub fn post_iter(root: &Id, f: &Fn(&Component)) {
    iter(root, IterMode::POST, f);
}
pub fn pre_iter(root: &Id, f: &Fn(&Component)) {
    iter(root, IterMode::PRE, f);
}

pub trait RootComponent : Component {}
