use na::Matrix3;

use super::{VMat, FMat, Vertex, Face, MeshAlloc};

use std::{
    fmt::Debug,
    mem::size_of,
};

#[derive(Clone, Debug)]
pub struct TriMeshGeom {
    // Used for loading optimization.
    pub mesh_id: u64,
    pub vv: VMat,
    pub ff: FMat,
    pub vec_vv: Vec<Vertex>,
    pub vec_ff: Vec<Face>,
    pub tex_file: Option<String>,
}
impl TriMeshGeom {
    pub fn new(mesh_alloc: &mut MeshAlloc, vv: VMat, ff: FMat, vertex_norms: Vec<[f32; 3]>, face_norms: Vec<[f32; 3]>, uvs: Vec<[f32; 2]>, texture_file: Option<String>) -> Self {
        Self {
            mesh_id: mesh_alloc.alloc_id(),
            vec_vv: {
                let mut vec_vv = Vec::with_capacity(vv.ncols() * size_of::<Vertex>());
                for c in 0..vv.ncols() {
                    let pos = vv.column(c);
                    vec_vv.push(Vertex {
                        pos: [pos[0], pos[1], pos[2]],
                        norm: vertex_norms[c],
                        uv: uvs[c],
                    });
                }
                vec_vv
            },
            vec_ff: {
                let mut vec_ff = Vec::with_capacity(ff.ncols() * size_of::<Face>());
                for c in 0..ff.ncols() {
                    let idxs = ff.column(c);
                    vec_ff.push(Face {
                        indices: [idxs[0], idxs[1], idxs[2]],
                        norm: face_norms[c],
                    });
                }
                vec_ff
            },
            vv,
            ff,
            tex_file: texture_file,
        }
    }

    pub fn triangle(
        mesh_alloc: &mut MeshAlloc,
        // Assumes this is in order
        vv: Matrix3<f32>,
        vertex_norms: [[f32; 3]; 3],
        uvs: [[f32; 2]; 3],
        face_norm: [f32; 3],
        texture_file: Option<String>,
    ) -> Self {
        Self {
            mesh_id: mesh_alloc.alloc_id(),
            vv: {
                VMat::from(vv.columns(0, 3))
            },
            ff: {
                FMat::from_iterator(1, 0..3)
            },
            vec_vv: {
                let mut vec_vv = Vec::with_capacity(vv.ncols() * size_of::<Vertex>());
                for c in 0..vv.ncols() {
                    let pos = vv.column(c);
                    vec_vv.push(Vertex {
                        pos: [pos[0], pos[1], pos[2]],
                        norm: vertex_norms[c],
                        uv: uvs[c],
                    });
                }
                vec_vv
            },
            vec_ff: vec![Face {
                indices: [0, 1, 2],
                norm: face_norm,
            }],
            tex_file: texture_file,
        }
    }
}
