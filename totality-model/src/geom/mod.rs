pub mod tri;
pub mod tet;

use std::{
    fmt::Debug,
    mem::size_of,
};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use na::{Dynamic, Matrix, VecStorage, U3};

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
            elemsize: size_of::<[f32; 3]>(),
        };
        let uv = VertexInfo {
            offset: size_of::<[f32; 3]>(),
            elemsize: size_of::<[f32; 2]>(),
        };
        vec![pos, uv]
    }
    pub fn packed_elem_sz() -> usize {
        size_of::<u32>()
    }
    pub fn packed_flat_sz() -> usize {
        5
    }
    pub fn packed_byte_sz() -> usize {
        Self::packed_flat_sz() * Self::packed_elem_sz()
    }
    fn pack_into(&self, buf: &mut Vec<u32>) {
        for p_d in self.pos.iter() {
            buf.push(p_d.clone().to_bits());
        }
        for uv_d in self.uv.iter() {
            buf.push(uv_d.clone().to_bits());
        }
    }
    fn packed_sz_float() -> usize {
        5 * size_of::<f32>()
    }
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
    pub fn packed_elem_sz() -> usize {
        size_of::<u32>()
    }
    pub fn packed_flat_sz() -> usize {
        3
    }
    pub fn packed_byte_sz() -> usize {
        Self::packed_flat_sz() * Self::packed_elem_sz()
    }
    fn pack_into(&self, buf: &mut Vec<u32>) {
        for v_i_d in self.verts.iter() {
            buf.push(v_i_d.clone());
        }
    }
}

pub type VMat = Matrix<f32, U3, Dynamic, VecStorage<f32, U3, Dynamic>>;
pub type FMat = Matrix<u32, U3, Dynamic, VecStorage<u32, U3, Dynamic>>;

pub trait Geom: Send + Sync + Debug {
    fn verts(&self) -> &VMat;
    fn faces(&self) -> &FMat;
    fn culled_verts(&self, intersect_test: Box<dyn IntersectTestable>) -> VMat {
        self.verts().clone()
    }
    fn culled_faces(&self, intersect_test: Box<dyn IntersectTestable>) -> FMat {
        self.faces().clone()
    }
    fn vert_cnt(&self) -> usize {
        self.verts().ncols()
    }
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
    fn vv_elem_cnt(&self) -> usize {
        self.verts().ncols()
    }
    fn ff_elem_cnt(&self) -> usize {
        self.faces().ncols()
    }
    fn vv_flat_cnt(&self) -> usize {
        self.vv_elem_cnt() * Vertex::packed_flat_sz()
    }
    fn ff_flat_cnt(&self) -> usize {
        self.ff_elem_cnt() * Face::packed_flat_sz()
    }
    fn vv_byte_cnt(&self) -> usize {
        self.vv_elem_cnt() * Vertex::packed_byte_sz()
    }
    fn ff_byte_cnt(&self) -> usize {
        self.ff_elem_cnt() * Face::packed_byte_sz()
    }
    fn texture(&self) -> &Option<String>;
    fn has_texture(&self) -> bool {
        self.texture().is_some()
    }
}

pub trait IntersectTestable {
    fn intersects(&self, t: Box<dyn Geom>) -> Option<()>;
}

