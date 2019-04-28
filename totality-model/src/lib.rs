extern crate nalgebra as na;
extern crate arrayvec as av;
extern crate log;

pub mod scene;
pub mod camera;

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
#[allow(dead_code)]
use log::{trace, info, debug, warn, error};

pub type VMat = Matrix<f32, U3, Dynamic, VecStorage<f32, U3, Dynamic>>;
pub type FMat = Matrix<u32, U3, Dynamic, VecStorage<u32, U3, Dynamic>>;
pub trait IntersectTestable {
    fn intersects(&self, t: Box<Geom>) -> Option<()>;
}
pub trait Geom: Send + Sync {
    fn verts(&self) -> &VMat;
    fn faces(&self) -> &FMat;
    fn culled_verts(&self, intersect_test: Box<IntersectTestable>) -> VMat { self.verts().clone() }
    fn culled_faces(&self, intersect_test: Box<IntersectTestable>) -> FMat { self.faces().clone() }
    fn vert_cnt(&self) -> usize { self.verts().ncols() }
    fn unpacked_verts(&self) -> &Vec<Vertex>;
    fn unpacked_faces(&self) -> &Vec<Face>;
    fn flattened_verts_as_bytes(&self) -> Vec<u32> {
        let mut flat = Vec::with_capacity(self.vv_flat_cnt());
        for v in self.unpacked_verts().iter() {
            v.pack_into(&mut flat);
        }
        flat
    }
    fn flattened_verts_as_floats(&self) -> Vec<f32> {
        let mut flat = Vec::with_capacity(self.vv_flat_cnt());
        for v in self.unpacked_verts().iter() {
            v.pack_into_float(&mut flat);
        }
        flat
    }
    fn flattened_faces_as_bytes(&self) -> Vec<u32> {
        let mut flat = Vec::with_capacity(self.ff_flat_cnt());
        for f in self.unpacked_faces().iter() {
            f.pack_into(&mut flat);
        }
        flat
    }
    fn vv_elem_cnt(&self) -> usize { self.verts().ncols() }
    fn ff_elem_cnt(&self) -> usize { self.faces().ncols() }
    fn vv_flat_cnt(&self) -> usize { self.vv_elem_cnt() * Vertex::packed_flat_sz() }
    fn ff_flat_cnt(&self) -> usize { self.ff_elem_cnt() * Face::packed_flat_sz() }
    fn vv_byte_cnt(&self) -> usize { self.vv_elem_cnt() * Vertex::packed_byte_sz() }
    fn ff_byte_cnt(&self) -> usize { self.ff_elem_cnt() * Face::packed_byte_sz() }
    fn texture(&self) -> &Option<String>;
    fn has_texture(&self) -> bool { self.texture().is_some() }
}

// TODO should this be a trait?
#[derive(Clone)]
pub struct Model {
    pos: Vector3<f32>,
    vel: Vector3<f32>,
    ori: UnitQuaternion<f32>,
    omg: UnitQuaternion<f32>,
    pub source: Arc<Box<Geom>>,
    children: Option<Arc<Vec<Arc<Model>>>>,
    parent: Option<Weak<Model>>,
}
impl Model {
    pub fn from_geom(g: Arc<Box<Geom>>) -> Model {
        Model {
            pos: Vector3::zeros(),
            vel: Vector3::zeros(),
            ori: UnitQuaternion::identity(),
            omg: UnitQuaternion::identity(),
            source: g.clone(),
            children: Option::None,
            parent: Option::None
        }
    }

    pub fn set_state(&mut self, p: Vector3<f32>, v: Vector3<f32>, o: UnitQuaternion<f32>, omg: UnitQuaternion<f32>) {
        self.set_pos(p);
        self.set_vel(v);
        self.set_ori(o);
        self.set_omg(omg);
    }
    pub fn set_pos(&mut self, p: Vector3<f32>) {
        self.pos = p;
    }
    pub fn set_vel(&mut self, v: Vector3<f32>) {
        self.vel = v;
    }
    pub fn set_ori(&mut self, o: UnitQuaternion<f32>) {
        self.ori = o;
    }
    pub fn set_omg(&mut self, o: UnitQuaternion<f32>) {
        self.omg = o;
    }

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
    pub fn packed_elem_sz() -> usize { size_of::<u32>() }
    pub fn packed_flat_sz() -> usize { 5 }
    pub fn packed_byte_sz() -> usize { Self::packed_flat_sz() * Self::packed_elem_sz() }
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
    pub fn packed_elem_sz() -> usize { size_of::<u32>() }
    pub fn packed_flat_sz() -> usize { 3 }
    pub fn packed_byte_sz() -> usize { Self::packed_flat_sz() * Self::packed_elem_sz() }
    fn pack_into(&self, buf: &mut Vec<u32>) {
        for v_i_d in self.verts.iter() {
            buf.push(v_i_d.clone());
        }
    }
}

