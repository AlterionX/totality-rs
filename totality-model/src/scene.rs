use crate::{Model, geom::tri::TriMeshGeom};
use std::sync::Arc;

#[derive(Debug)]
pub struct Static {
    pub objs: Vec<Arc<Box<TriMeshGeom>>>,
}
#[derive(Debug, Clone)]
pub struct Dynamic {
    pub mm: Vec<Model>,
}

pub struct Scene(Static, Dynamic);
impl Scene {
    pub fn new(gg: Vec<Arc<Box<TriMeshGeom>>>, mm: Vec<Model>) -> (Static, Dynamic) {
        (Static { objs: gg }, Dynamic { mm })
    }
    pub fn split(self) -> (Static, Dynamic) {
        (self.0, self.1)
    }
    pub fn rejoin(st: Static, dy: Dynamic) -> Self {
        Self(st, dy)
    }
}

