use std::{sync::{Arc, RwLock, Mutex}, mem::size_of};
use super::{Geom, Vertex, VMat, FMat, Model, Face};
use na::{Matrix3, Vector3, Matrix, U2, U3};

#[allow(dead_code)]
use log::{trace, debug, info, warn, error};

pub struct Static {
    pub objs: Vec<Arc<Box<Geom>>>,
}
pub struct Dynamic {
    pub mm: Vec<Model>,
}

#[derive(Debug, Copy, Clone)]
struct TripleBufferIdx {
    snatched: usize,
    most_recent: usize,
    curr_write: usize,
    next_write: usize,
}

pub struct Scene {
    pub statics: Arc<Static>,
    pub dynamics: [Arc<RwLock<Dynamic>>; 3],
    indices: Mutex<TripleBufferIdx>,
}
impl Scene {
    pub fn new(gg: Vec<Arc<Box<Geom>>>, mm: Vec<Model>) -> Scene {
        Scene {
            statics: Arc::new(Static { objs: gg }),
            dynamics: [
                Arc::new(RwLock::new(Dynamic { mm: mm.clone() })),
                Arc::new(RwLock::new(Dynamic { mm: mm.clone() })),
                Arc::new(RwLock::new(Dynamic { mm: mm.clone() })),
            ],
            indices: Mutex::new(TripleBufferIdx {
                snatched: 0usize,
                most_recent: 0usize,
                curr_write: 1usize,
                next_write: 2usize,
            }),
        }
    }
    pub fn snatch(&self) -> Arc<RwLock<Dynamic>> {
        if let Ok(mut indices) = self.indices.lock() {
            trace!("Snatching indices: {:?}.", indices);
            if indices.snatched != indices.most_recent {
                indices.next_write = indices.snatched;
                indices.snatched = indices.most_recent;
            }
            trace!("Reached indices: {:?}.", indices);
            self.dynamics[indices.snatched].clone()
        } else { panic!("Poisoned buffer indices!") }
    }
    pub fn advance(&self) -> (Arc<RwLock<Dynamic>>, Arc<RwLock<Dynamic>>) {
        if let Ok(mut indices) = self.indices.lock() {
            trace!("Advancing indices: {:?}.", indices);
            indices.most_recent = indices.curr_write;
            indices.curr_write = indices.next_write;
            indices.next_write = indices.most_recent;
            trace!("Reached indices: {:?}.", indices);
            (self.dynamics[indices.most_recent].clone(), self.dynamics[indices.curr_write].clone())
        } else { panic!("Poisoned buffer indices!") }
    }
}

#[derive(Clone)]
pub struct TriGeom {
    vv: VMat,
    ff: FMat,
    vec_vv: Vec<Vertex>,
    vec_ff: Vec<Face>,
    tex_file: Option<String>,
}
impl TriGeom {
    pub fn new(vv: Matrix3<f32>, f: Vector3<u32>, uvs: Vec<[f32; 2]>, texture_file: Option<String>) -> TriGeom {
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
    fn verts(&self) -> &VMat { &self.vv }
    fn faces(&self) -> &FMat { &self.ff }
    fn unpacked_verts(&self) -> &Vec<Vertex> { &self.vec_vv }
    fn unpacked_faces(&self) -> &Vec<Face> { &self.vec_ff }
    fn texture(&self) -> &Option<String> { &self.tex_file }
}

#[derive(Clone)]
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
    fn verts(&self) -> &VMat { &self.vv }
    fn faces(&self) -> &FMat { &self.ff }
    fn unpacked_verts(&self) -> &Vec<Vertex> { &self.vec_vv }
    fn unpacked_faces(&self) -> &Vec<Face> { &self.vec_ff }
    fn texture(&self) -> &Option<String> { &self.tex_file }
}

