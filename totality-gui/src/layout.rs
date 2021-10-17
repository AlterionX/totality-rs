use crate::component::Component;
use std::{
    cmp::{max, min},
    rc::Rc,
};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
struct Dim {
    pub hori: u64,
    pub vert: u64,
}
impl Dim {
    fn max(d0: &Self, d1: &Self) -> Self {
        Self {
            hori: max(d0.hori, d1.hori),
            vert: max(d0.vert, d0.vert),
        }
    }
    fn min(d0: &Self, d1: &Self) -> Self {
        Self {
            hori: min(d0.hori, d1.hori),
            vert: min(d0.vert, d0.vert),
        }
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Pos(Dim);
impl Pos {
    pub fn new(w: u64, h: u64) -> Pos {
        Pos(Dim { hori: w, vert: h })
    }
    pub fn w(&self) -> &u64 {
        &self.0.hori
    }
    pub fn h(&self) -> &u64 {
        &self.0.vert
    }
}
impl From<Dim> for Pos {
    fn from(d: Dim) -> Self {
        Self(d)
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Sz(Dim);
impl Sz {
    pub fn new(x: u64, y: u64) -> Sz {
        Sz(Dim { hori: x, vert: y })
    }
    pub fn x(&self) -> &u64 {
        &self.0.hori
    }
    pub fn y(&self) -> &u64 {
        &self.0.vert
    }
}
impl From<Dim> for Sz {
    fn from(d: Dim) -> Self {
        Self(d)
    }
}
#[derive(Debug, Default, Copy, Clone)]
pub struct Rot(f64);
impl Rot {
    pub fn new(theta: f64) -> Self {
        Self(theta)
    }
    pub fn theta(&self) -> &f64 {
        &self.0
    }
}
impl From<f64> for Rot {
    fn from(theta: f64) -> Self {
        Rot(theta)
    }
}
#[derive(Debug, Default, Copy, Clone)]
pub struct Cfg(pub Pos, pub Sz, pub Rot);
impl Cfg {
    pub fn p(&self) -> &Pos {
        &self.0
    }
    pub fn s(&self) -> &Sz {
        &self.1
    }
    pub fn r(&self) -> &Rot {
        &self.2
    }
}
impl From<(Pos, Sz, Rot)> for Cfg {
    fn from(tup: (Pos, Sz, Rot)) -> Self {
        Self(tup.0, tup.1, tup.2)
    }
}

pub trait Placer {
    fn place(&self, comp: &Vec<Rc<Box<dyn Component>>>, sz: Sz) -> Vec<Cfg>;
}

pub struct LiteralPlacer {
    cfgs: Vec<Cfg>,
}
impl LiteralPlacer {
    pub fn new() -> Self {
        Self { cfgs: vec![] }
    }
    pub fn set_placements(&mut self, mut cfgs: Vec<Cfg>) {
        std::mem::swap(&mut self.cfgs, &mut cfgs);
    }
}
impl Placer for LiteralPlacer {
    fn place(&self, comp: &Vec<Rc<Box<dyn Component>>>, sz: Sz) -> Vec<Cfg> {
        self.cfgs.clone()
    }
}
