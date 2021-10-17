pub mod geom;
pub mod camera;
pub mod scene;

use geom::Geom;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use na::{Matrix4, UnitQuaternion, Vector3, U1, U3};
use std::{
    fmt::Debug,
    sync::{Arc, Weak},
};

// TODO should this be a trait?
#[derive(Debug, Clone)]
pub struct Model {
    pub pos: Vector3<f32>,
    pub vel: Vector3<f32>,
    pub ori: UnitQuaternion<f32>,
    pub omg: UnitQuaternion<f32>,
    pub scale: f32,
    pub source: Arc<Box<dyn Geom>>,
    should_render: bool,
    children: Option<Arc<Vec<Arc<Model>>>>,
    parent: Option<Weak<Model>>,
}
impl Model {
    pub fn from_geom(g: Arc<Box<dyn Geom>>) -> Model {
        Model {
            pos: Vector3::zeros(),
            vel: Vector3::zeros(),
            ori: UnitQuaternion::identity(),
            omg: UnitQuaternion::identity(),
            scale: 1.,
            source: g.clone(),
            should_render: false,
            children: Option::None,
            parent: Option::None,
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
        self.set_pos(p);
        self.set_vel(v);
        self.set_ori(o);
        self.set_omg(omg);
        self.set_scale(scale);
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
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    pub fn set_should_render(&mut self, b: bool) {
        self.should_render = b;
    }

    pub fn mat(&self) -> Matrix4<f32> {
        let s = self.scale;
        let mut t_mat = self.ori.to_homogeneous() * Matrix4::from_partial_diagonal(&[s, s, s, 1.0]);
        t_mat.fixed_slice_mut::<U3, U1>(0, 3).copy_from(&self.pos);
        t_mat
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

