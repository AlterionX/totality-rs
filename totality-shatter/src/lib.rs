extern crate nalgebra as na;
extern crate boomphf as phf;
extern crate totality_model as model;

mod adv;
mod shatter;
mod sorted_tri;

use sorted_tri as st;

use model::{Model, geom::tet::TetGeom};

use std::sync::Arc;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use na::{DMatrix, DefaultAllocator, Dynamic, Vector3, U1, U3};

pub struct ShatterModel {
    pub model: Model,
    pub geom: Arc<ShatterGeom>,
    pub broken: st::SortedTri,
    pub alp: DMatrix<f64>,
    pub vel: DMatrix<f64>,
}

pub struct ShatterGeom {
    pub geom: Arc<model::geom::tet::TetGeom>,
    pub shatterable: bool,
}

impl ShatterModel {
    fn advance(&mut self) {
        loop {
            if !shatter::fracture(self) && !adv::update(&self.vel, &self.alp) {
                break
            }
        }
    }
    fn can_shatter(&self) -> bool {
        0 == 0
    }
    fn shatter(&mut self) -> Vec<TetGeom> {
        // TODO wrap output into ShatterModel
        shatter::shatter(self)
    }
}
