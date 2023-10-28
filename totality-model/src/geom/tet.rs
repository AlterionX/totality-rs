#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};
use na::{Dynamic, Matrix, VecStorage, U3, U4};

use super::tri::TriMeshGeom;

pub type TMat = Matrix<u32, U3, Dynamic, VecStorage<u32, U4, Dynamic>>;

pub struct TetGeom {
    mesh: TriMeshGeom,
    tets: TMat,
}
impl TetGeom {
    pub fn submeshes() -> u64 {
        0
    }
}

