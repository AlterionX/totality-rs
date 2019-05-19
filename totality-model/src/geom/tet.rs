use super::{VMat, FMat, Geom, Vertex, Face, tri::TriGeom};

use std::{
    fmt::Debug,
    mem::size_of,
    sync::{Arc, Weak},
};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use na::{Dynamic, Matrix, Matrix3, UnitQuaternion, VecStorage, Vector3, U1, U3, U4};

pub type TMat = Matrix<u32, U3, Dynamic, VecStorage<u32, U4, Dynamic>>;

pub struct TetGeom {
    mesh: TriGeom,
    tets: TMat,
}
impl TetGeom {
    pub fn submeshes() -> u64 {
        0
    }
}

