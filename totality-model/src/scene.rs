use crate::{
    Model,
    geom::{FMat, Face, Geom, VMat, Vertex}
};
use na::{Matrix, Matrix3, Vector3, U2, U3};
use std::{
    mem::size_of,
    sync::{Arc, Mutex, RwLock},
};

#[allow(dead_code)]
use log::{debug, error, info, trace, warn};

#[derive(Debug)]
pub struct Static {
    pub objs: Vec<Arc<Box<Geom>>>,
}
#[derive(Debug, Clone)]
pub struct Dynamic {
    pub mm: Vec<Model>,
}

pub struct Scene(Static, Dynamic);
impl Scene {
    pub fn new(gg: Vec<Arc<Box<Geom>>>, mm: Vec<Model>) -> (Static, Dynamic) {
        (Static { objs: gg }, Dynamic { mm: mm })
    }
    pub fn split(self) -> (Static, Dynamic) {
        (self.0, self.1)
    }
    pub fn rejoin(st: Static, dy: Dynamic) -> Self {
        Self(st, dy)
    }
}

