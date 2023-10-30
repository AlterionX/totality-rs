pub mod geom;
pub mod camera;
pub mod scene;

use geom::{tri::TriMeshGeom, MeshAlloc};

use na::{Matrix4, UnitQuaternion, Vector3, Vector4};
use std::{
    fmt::Debug,
    sync::{Arc, Weak},
};

#[derive(Debug, Clone)]
pub struct AffineTransform {
    pub pos: Vector3<f32>,
    pub ori: UnitQuaternion<f32>,
    pub scaling: Vector4<f32>,
}

impl AffineTransform {
    pub fn identity() -> Self {
        Self {
            pos: Vector3::zeros(),
            ori: UnitQuaternion::identity(),
            scaling: Vector4::new(1., 1., 1., 1.),
        }
    }

    pub fn mat(&self) -> Matrix4<f32> {
        let mut t_mat = Matrix4::from_diagonal(&self.scaling) * self.ori.to_homogeneous();
        t_mat.fixed_view_mut::<3, 1>(0, 3).copy_from(&self.pos);
        t_mat
    }
}

// TODO should this be a trait?
#[derive(Debug, Clone)]
pub struct Model {
    pub transform: AffineTransform,

    pub source: Arc<TriMeshGeom>,

    pub omg: UnitQuaternion<f32>,
    children: Option<Arc<Vec<Arc<Model>>>>,
    parent: Option<Weak<Model>>,

    should_render: bool,
}
impl Model {
    pub fn from_geom(g: Arc<TriMeshGeom>) -> Model {
        Model {
            transform: AffineTransform::identity(),

            source: g.clone(),

            omg: UnitQuaternion::identity(),
            children: Option::None,
            parent: Option::None,

            should_render: false,
        }
    }

    pub fn set_state(
        &mut self,
        p: Vector3<f32>,
        v: Vector3<f32>,
        o: UnitQuaternion<f32>,
        omg: UnitQuaternion<f32>,
        scale: f32,
    ) {
        self.set_omg(omg);
    }
    pub fn set_omg(&mut self, o: UnitQuaternion<f32>) {
        self.omg = o;
    }

    pub fn set_should_render(&mut self, b: bool) {
        self.should_render = b;
    }
}
unsafe impl Send for Model {}

/// Generates the mesh of a unit cube, centered on the origin.
pub fn unit_cube(alloc: &mut MeshAlloc, texture: Option<String>) -> TriMeshGeom {
    TriMeshGeom::new(
        alloc,
        geom::VMat::from_iterator(
            8,
            [
                -0.5, -0.5, -0.5, // left bottom rear
                -0.5, -0.5,  0.5, // left bottom front
                -0.5,  0.5, -0.5, // left top rear
                -0.5,  0.5,  0.5, // left top front
                 0.5, -0.5, -0.5, // right bottom rear
                 0.5, -0.5,  0.5, // right bottom front
                 0.5,  0.5, -0.5, // right top rear
                 0.5,  0.5,  0.5, // right top front
            ]
            .into_iter(),
        ),
        geom::FMat::from_iterator(
            12,
            vec![
                1, 0, 4, 5, 1, 4, // bottom
                6, 2, 3, 6, 3, 7, // top
                0, 1, 2, 3, 2, 1, // left
                4, 6, 7, 4, 7, 5, // right
                0, 2, 6, 0, 6, 4, // back
                5, 7, 3, 3, 1, 5, // front
            ]
            .into_iter(),
        ),
        vec![[0.0, 0.0, 0.0]; 8],
        vec![[0.0, 0.0, 0.0]; 12],
        vec![
            [0f32, 0f32],
            [0f32, 1f32],
            [0f32, 0f32],
            [0f32, 1f32],
            [0f32, 0f32],
            [0f32, 1f32],
            [0f32, 0f32],
            [0f32, 1f32],
        ],
        texture,
    )
}
