use std::sync::Arc;
use super::{Geom, VMat, FMat, Model};
use na::{Matrix3, Vector3};


pub struct Scene {
    objs: Vec<Arc<Box<Geom>>>,
    mm: Vec<Model>,
}
impl Scene {
    pub fn new(gg: Vec<Arc<Box<Geom>>>, mm: Vec<Model>) -> Scene {
        Scene {
            objs: gg,
            mm: mm
        }
    }
}

#[derive(Clone)]
pub struct TriGeom {
    vv: VMat,
    ff: FMat,
}
impl TriGeom {
    pub fn new(vv: Matrix3<f32>, f: Vector3<u32>) -> TriGeom {
        TriGeom {
            vv: {
                let mut vv_ = unsafe { VMat::new_uninitialized(3) };
                vv_.copy_from(&vv);
                vv_
            },
            ff: {
                let mut ff_ = unsafe { FMat::new_uninitialized(1) };
                ff_.copy_from(&f);
                ff_
            },
        }
    }
}
impl Geom for TriGeom {
    fn verts(&self) -> &VMat { &self.vv }
    fn faces(&self) -> &FMat { &self.ff }
}

