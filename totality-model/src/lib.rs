extern crate nalgebra as na;
extern crate arrayvec as av;
extern crate log;

pub mod scene;

use std::sync::{Weak, Arc};
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
}
// TODO should this be a trait?
#[derive(Clone)]
pub struct Model {
    iso: Isometry3<f32>,
    source: Arc<Box<Geom>>,
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
        let vv = self.source.verts();
        let mut flattened = Vec::with_capacity(vv.nrows() * 3);
        for i in 0..vv.ncols() {
            let p = vv.fixed_columns::<U1>(i);
            flattened.extend(vec![p[0], p[1], p[2]]);
        }
        flattened
    }
    pub fn transformed_flat_v(&self) -> Vec<f32> {
        let vv = self.source.verts();
        let mut flattened = Vec::with_capacity(vv.nrows() * 3);
        for i in 0..vv.ncols() {
            let p_o = Point3::from(Vector3::from(vv.fixed_columns::<U1>(i)));
            let p = self.iso * p_o;
            trace!("Original point: {:?}; Transformed point: {:?}", p_o, p);
            flattened.extend(vec![p[0], p[1], p[2]]);
        }
        flattened
    }
}
unsafe impl Send for Model {}

