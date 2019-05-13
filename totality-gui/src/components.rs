use std::rc::Rc;
use crate::color::{self, Color};
use crate::layout::Sz;
use crate::event as e;
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

pub struct Id(u64, Rc<Component>);
impl Id {
    pub fn get(&self) -> &Component { &*self.1 }
    pub fn get_id(&self) -> u64 { self.0 }
}

pub struct ChildrenInfo {
    pub placer: Box<Placer>,
    pub children: Vec<Id>,
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SizingInfo {
    pub min: Option<Sz>,
    pub max: Option<Sz>,
    pub preferred: Option<Sz>,
}
pub trait Component {
    // Mostly preset data
    fn sz_info(&self) -> Option<&SizingInfo>;
    fn min_sz(&self) -> Option<&Sz> { self.sz_info().and_then(|si| si.min.as_ref()) }
    fn max_sz(&self) -> Option<&Sz> { self.sz_info().and_then(|si| si.max.as_ref()) }
    fn preferred_sz(&self) -> Option<&Sz> { self.sz_info().and_then(|si| si.preferred.as_ref()) }
    fn children_info(&self) -> Option<&ChildrenInfo>;
    fn placer(&self) -> Option<&Box<Placer>> { self.children_info().map(|ci| &ci.placer) }
    fn children(&self) -> Option<&Vec<Id>> {  self.children_info().map(|ci| &ci.children) }
    // other stuff
    fn parent(&self) -> Option<&Id>;
    fn bg(&self) -> Option<Background> { Some(Background::Color(*color::TRANSPARENT)) }
    // Changes dynamically
    fn set_placer(&self, p: &Box<Placer>);
    fn resize(&self, sz: Sz);
    fn fire_event(&mut self, e: &e::E) -> ShouldHaltPropagation { false.into() }
    fn assign_listener(&self); // TODO figure out how this one works
    // draw
    fn set_dirty(&mut self);
    fn need_redraw(&self) -> bool { true }
    fn draw(&self) -> Vec<DrawCmd>;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum IterMode { POST, PRE }
fn iter(root: &Id, mode: IterMode, f: &Fn(&Component)) {
    if mode == IterMode::PRE { f(root.get()); }
    if let Some(cc) = root.get().children() {
        for child in cc.iter() {
            pre_iter(child, f);
        }
    }
    if mode == IterMode::POST { f(root.get()); }
}
pub fn post_iter(root: &Id, f: &Fn(&Component)) {
    iter(root, IterMode::POST, f);
}
pub fn pre_iter(root: &Id, f: &Fn(&Component)) {
    iter(root, IterMode::PRE, f);
}

pub trait RootComponent : Component {
    fn root_placer() -> Box<Placer>;
}