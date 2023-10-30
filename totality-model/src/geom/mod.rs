pub mod tri;
pub mod tet;

use std::{
    fmt::Debug,
    mem::size_of,
};

use na::Matrix3xX;

#[derive(Debug, Copy, Clone, bytemuck::Zeroable)]
#[repr(C, packed)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub norm: [f32; 3],
    pub uv: [f32; 2],
}

unsafe impl bytemuck::Pod for Vertex {}

#[derive(Debug, Copy, Clone, bytemuck::Zeroable)]
#[repr(C, packed)]
pub struct Face {
    pub indices: [u32; 3],
    pub norm: [f32; 3],
}

unsafe impl bytemuck::Pod for Face {}

#[derive(Debug, Copy, Clone, bytemuck::Zeroable)]
#[repr(C, packed)]
pub struct Material {
    pub emission: [f32; 3],
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub specular: [f32; 3],
    pub shininess: f32,
    pub transparent: bool,
}

unsafe impl bytemuck::Pod for Material {}

pub type VMat = Matrix3xX<f32>;
pub type FMat = Matrix3xX<u32>;

pub struct MeshAlloc {
    counter: u64,
}

impl MeshAlloc {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
    pub (crate) fn alloc_id(&mut self) -> u64 {
        let c = self.counter;
        self.counter += 1;
        c
    }
}
