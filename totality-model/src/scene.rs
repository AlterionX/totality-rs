use std::{sync::Arc, mem::size_of};
use super::{Geom, Vertex, VMat, FMat, Model, Face};
use na::{Matrix3, Vector3, Matrix, U2, U3};


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
    vec_vv: Vec<Vertex>,
    vec_ff: Vec<Face>,
}
impl TriGeom {
    pub fn new(vv: Matrix3<f32>, f: Vector3<u32>, uvs: Vec<[f32; 2]>) -> TriGeom {
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
            vec_vv: {
                let mut vec_vv = Vec::with_capacity(vv.ncols() * size_of::<Vertex>());
                for c in 0..vv.ncols() {
                    vec_vv.push(Vertex {
                            pos: vv.column(c).into(),
                            uv: uvs[c],
                    });
                }
                vec_vv
            },
            vec_ff: vec![Face { verts: f.into() }],
        }
    }
}
impl Geom for TriGeom {
    fn verts(&self) -> &VMat { &self.vv }
    fn faces(&self) -> &FMat { &self.ff }
    fn unpacked_verts(&self) -> &Vec<Vertex> { &self.vec_vv }
    fn unpacked_faces(&self) -> &Vec<Face> { &self.vec_ff }
}

