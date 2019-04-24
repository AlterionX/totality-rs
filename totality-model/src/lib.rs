extern crate nalgebra as na;
extern crate arrayvec as av;
extern crate log;

pub mod scene;

use std::{
    mem::size_of,
    sync::{Weak, Arc},
};
use na::{
    U1, U3, Dynamic,
    Vector3,
    UnitQuaternion, Point3,
    Rotation3, Isometry3, Translation3,
    Matrix,
    VecStorage,
};
use log::*;

pub type VMat = Matrix<f32, U3, Dynamic, VecStorage<f32, U3, Dynamic>>;
pub type FMat = Matrix<u32, U3, Dynamic, VecStorage<u32, U3, Dynamic>>;
pub trait IntersectTestable {
    fn intersects(&self, t: Box<Geom>) -> Option<()>;
}
pub trait Geom {
    fn verts(&self) -> &VMat;
    fn faces(&self) -> &FMat;
    fn culled_verts(&self, intersect_test: Box<IntersectTestable>) -> VMat { self.verts().clone() }
    fn culled_faces(&self, intersect_test: Box<IntersectTestable>) -> FMat { self.faces().clone() }
    fn vert_cnt(&self) -> usize { self.verts().ncols() }
    fn unpacked_verts(&self) -> &Vec<Vertex>;
    fn unpacked_faces(&self) -> &Vec<Face>;
    fn flattened_verts_as_bytes(&self) -> Vec<u32> {
        let mut flat = Vec::with_capacity(self.vert_cnt() * Vertex::packed_sz());
        for v in self.unpacked_verts().iter() {
            v.pack_into(&mut flat);
        }
        flat
    }
    fn flattened_verts_as_floats(&self) -> Vec<f32> {
        let mut flat = Vec::with_capacity(self.vert_cnt() * Vertex::packed_sz_float());
        for v in self.unpacked_verts().iter() {
            v.pack_into_float(&mut flat);
            trace!("Inserting vert {:?}", v);
        }
        flat
    }
    fn flattened_faces_as_bytes(&self) -> Vec<u32> {
        let mut flat = Vec::with_capacity(self.vert_cnt() * Vertex::packed_sz());
        for f in self.unpacked_faces().iter() {
            f.pack_into(&mut flat);
        }
        flat
    }
    fn n_vv(&self) -> usize { self.verts().ncols() }
    fn n_ff(&self) -> usize { self.faces().ncols() }
    fn packed_vv_sz(&self) -> usize { self.n_vv() * Vertex::packed_sz() }
    fn packed_ff_sz(&self) -> usize { self.n_ff() * Face::packed_sz() }
}
// TODO should this be a trait?
#[derive(Clone)]
pub struct Model {
    iso: Isometry3<f32>,
    pub source: Arc<Box<Geom>>,
    children: Option<Arc<Vec<Arc<Model>>>>,
    parent: Option<Weak<Model>>,
}
impl Model {
    pub fn from_geom(g: Arc<Box<Geom>>) -> Model {
        Model {
            iso: Isometry3::identity(),
            source: g.clone(),
            children: Option::None,
            parent: Option::None
        }
    }
    pub fn m(&self) -> Isometry3<f32> { self.iso.clone() }
    pub fn off(&self) -> Translation3<f32> { self.iso.translation.clone() }
    pub fn off_v(&self) -> Vector3<f32> { self.iso.translation.vector.clone() }
    pub fn rot(&self) -> Rotation3<f32> { Rotation3::from(self.rot_q()) }
    pub fn rot_q(&self) -> UnitQuaternion<f32> { self.iso.rotation.clone() }
    pub fn set_m(&mut self, m: &Isometry3<f32>) { self.iso = m.clone(); }
    pub fn set_off(&mut self, offset: &Translation3<f32>) { self.iso.translation = offset.clone(); }
    pub fn set_off_v(&mut self, offset: &Vector3<f32>) {
        self.iso.translation = Translation3::from_vector(offset.clone());
    }
    pub fn set_rot(&mut self, rot: &Rotation3<f32>) {
        self.iso.rotation = UnitQuaternion::from_rotation_matrix(rot);
    }
    pub fn set_rot_q(&mut self, rot: &UnitQuaternion<f32>) { self.iso.rotation = rot.clone(); }

    pub fn flat_v(&self) -> Vec<f32> {
        self.source.flattened_verts_as_floats()
    }
    pub fn vv_as_bytes(&self) -> Vec<u32> {
        self.source.flattened_verts_as_bytes()
    }
    pub fn ff_as_bytes(&self) -> Vec<u32> {
        self.source.flattened_faces_as_bytes()
    }
}
unsafe impl Send for Model {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct VertexInfo {
    pub offset: usize,
    pub elemsize: usize,
}
#[derive(Debug, Copy, Clone)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
}
impl Vertex {
    pub fn attributes() -> Vec<VertexInfo> {
        let pos = VertexInfo {
            offset: 0,
            elemsize:  size_of::<[f32; 3]>(),
        };
        let uv = VertexInfo {
            offset: size_of::<[f32; 3]>(),
            elemsize:  size_of::<[f32; 2]>(),
        };
        vec![pos, uv]
    }
    pub fn packed_sz() -> usize { 5 * size_of::<u32>() }
    fn pack_into(&self, buf: &mut Vec<u32>) {
        for p_d in self.pos.iter() {
            buf.push(p_d.clone().to_bits());
        }
        for uv_d in self.uv.iter() {
            buf.push(uv_d.clone().to_bits());
        }
    }
    fn packed_sz_float() -> usize { 5 * size_of::<f32>() }
    fn pack_into_float(&self, buf: &mut Vec<f32>) {
        for p_d in self.pos.iter() {
            buf.push(p_d.clone());
        }
        for uv_d in self.uv.iter() {
            buf.push(uv_d.clone());
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Face {
    pub verts: [u32; 3],
}
impl Face {
    pub fn attributes() -> Vec<VertexInfo> {
        let verts = VertexInfo {
            offset: 0,
            elemsize: size_of::<[u32; 3]>(),
        };
        vec![verts]
    }
    pub fn packed_sz() -> usize { 3 * size_of::<u32>() }
    fn pack_into(&self, buf: &mut Vec<u32>) {
        for v_i_d in self.verts.iter() {
            buf.push(v_i_d.clone());
        }
    }
}
