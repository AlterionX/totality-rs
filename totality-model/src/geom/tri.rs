use super::{VMat, FMat, Geom, Vertex, Face};

use std::{
    fmt::Debug,
    mem::size_of,
    sync::{Arc, Weak},
};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use na::{Dynamic, Matrix, Matrix3, UnitQuaternion, VecStorage, Vector3, U1, U3, U4};

#[derive(Clone, Debug)]
pub struct TriGeom {
    vv: VMat,
    ff: FMat,
    vec_vv: Vec<Vertex>,
    vec_ff: Vec<Face>,
    tex_file: Option<String>,
}
impl TriGeom {
    pub fn new(
        vv: Matrix3<f32>,
        f: Vector3<u32>,
        uvs: Vec<[f32; 2]>,
        texture_file: Option<String>,
    ) -> TriGeom {
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
            tex_file: texture_file,
        }
    }
}
impl Geom for TriGeom {
    fn verts(&self) -> &VMat {
        &self.vv
    }
    fn faces(&self) -> &FMat {
        &self.ff
    }
    fn unpacked_verts(&self) -> &Vec<Vertex> {
        &self.vec_vv
    }
    fn unpacked_faces(&self) -> &Vec<Face> {
        &self.vec_ff
    }
    fn texture(&self) -> &Option<String> {
        &self.tex_file
    }
}

#[derive(Clone, Debug)]
pub struct TriMeshGeom {
    vv: VMat,
    ff: FMat,
    vec_vv: Vec<Vertex>,
    vec_ff: Vec<Face>,
    tex_file: Option<String>,
}
impl TriMeshGeom {
    pub fn new(vv: VMat, ff: FMat, uvs: Vec<[f32; 2]>, texture_file: Option<String>) -> TriGeom {
        TriGeom {
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
            vec_ff: {
                let mut vec_ff = Vec::with_capacity(ff.ncols() * size_of::<Face>());
                for c in 0..ff.ncols() {
                    vec_ff.push(Face {
                        verts: ff.column(c).into(),
                    });
                }
                vec_ff
            },
            vv: vv,
            ff: ff,
            tex_file: texture_file,
        }
    }
}
impl Geom for TriMeshGeom {
    fn verts(&self) -> &VMat {
        &self.vv
    }
    fn faces(&self) -> &FMat {
        &self.ff
    }
    fn unpacked_verts(&self) -> &Vec<Vertex> {
        &self.vec_vv
    }
    fn unpacked_faces(&self) -> &Vec<Face> {
        &self.vec_ff
    }
    fn texture(&self) -> &Option<String> {
        &self.tex_file
    }
}
