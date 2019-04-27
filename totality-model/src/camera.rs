use na::{UnitQuaternion, Vector3, Matrix4, U1, U3, U4};

#[derive(Debug, Copy, Clone)]
pub enum Camera {
    Perspective(PerspectiveCamera),
    Orthographic(OrthoCamera),
}
impl Camera {
    pub fn get_vp_mat(&self) -> Matrix4<f32> {
        match self {
            Camera::Perspective(cam) => {
                cam.vp_mat()
            },
            Camera::Orthographic(cam) => {
                cam.vp_mat()
            }
        }
    }
    pub fn trans_cam_space(&mut self, v: Vector3<f32>) {
        match self {
            Camera::Perspective(cam) => {
                cam.trans_cam_space(v)
            },
            Camera::Orthographic(cam) => {
                cam.trans_cam_space(v)
            }
        }
    }
    pub fn pos(&self) -> Vector3<f32> {
        match self {
            Camera::Perspective(cam) => {
                cam.pos()
            },
            Camera::Orthographic(cam) => {
                cam.pos()
            }
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct OrthoCamera {
    position: Vector3<f32>,
    orientation: UnitQuaternion<f32>,
    _p_cache: Matrix4<f32>,
    _v_cache: Matrix4<f32>,
}
impl OrthoCamera {
    pub fn v_mat(&self) -> Matrix4<f32> { self._v_cache }
    pub fn p_mat(&self) -> Matrix4<f32> { self._p_cache }
    pub fn vp_mat(&self) -> Matrix4<f32> { self.p_mat() * self.v_mat() }
    fn calc_p_mat(&mut self) { self._p_cache.fill_with_identity() }
    fn calc_v_mat(&mut self) {
        self._v_cache.fill_with_identity();
        self._v_cache.fixed_slice_mut::<U3, U1>(0, 3).copy_from(&(self.position * -1.0));
    }
    pub fn trans(&mut self, shift: Vector3<f32>) {
        self.position += shift;
        self.calc_v_mat();
    }
    pub fn trans_cam_space(&mut self, shift: Vector3<f32>) {
        self.trans(self.orientation.inverse_transform_vector(&shift));
    }
    pub fn pos(&self) -> Vector3<f32> { self.position }
}
impl Default for OrthoCamera {
    fn default() -> OrthoCamera {
        let mut cam = OrthoCamera {
            position: Vector3::zeros(),
            orientation: UnitQuaternion::identity(),
            _p_cache: Matrix4::zeros(),
            _v_cache: Matrix4::zeros(),
        };
        cam.calc_p_mat();
        cam.calc_v_mat();
        cam
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PerspectiveCamera {
    near_plane_dist: f32,
    far_plane_dist: f32,
    fov: f32,
    aspect: f32,
    orientation: UnitQuaternion<f32>,
    position: Vector3<f32>,
    _p_cache: Matrix4<f32>,
    _v_cache: Matrix4<f32>,
}
impl PerspectiveCamera {
    pub fn v_mat(&self) -> Matrix4<f32> { self._v_cache }
    pub fn p_mat(&self) -> Matrix4<f32> { self._p_cache }
    pub fn vp_mat(&self) -> Matrix4<f32> { self.v_mat() * self.p_mat() }
    fn calc_p_mat(&mut self) {
        let cot = 1.0 / (self.fov / 2.0).tan();
        let inv_depth = 1.0 / (self.far_plane_dist - self.near_plane_dist);
        self._p_cache = Matrix4::new(
            cot / self.aspect,  0f32,                                                   0f32, 0f32,
            0f32,                cot,                                                   0f32, 0f32,
            0f32,               0f32,                         self.far_plane_dist * inv_depth, 1f32,
            0f32,               0f32, -self.far_plane_dist * self.near_plane_dist * inv_depth, 1f32,
        );
    }
    fn calc_v_mat(&mut self) {
        self._v_cache = self.orientation.to_homogeneous();
        self._v_cache.fixed_slice_mut::<U3, U1>(0, 3).copy_from(&(self.position * -1.0));
    }
    pub fn rot(&mut self, rotor: UnitQuaternion<f32>) {
        self.orientation *= rotor;
        self.calc_v_mat();
    }
    pub fn trans(&mut self, shift: Vector3<f32>) {
        self.position += shift;
        self.calc_v_mat();
    }
    pub fn trans_cam_space(&mut self, shift: Vector3<f32>) {
        self.trans(self.orientation.inverse_transform_vector(&shift));
    }
    pub fn pos(&self) -> Vector3<f32> { self.position }
}
impl Default for PerspectiveCamera {
    fn default() -> Self {
        let mut cam = PerspectiveCamera {
            near_plane_dist: 0.01f32,
            far_plane_dist: 1000.0f32,
            fov: 90.0f32,
            aspect: 1.0f32,
            orientation: UnitQuaternion::identity(),
            position: Vector3::zeros(),
            _p_cache: Matrix4::zeros(),
            _v_cache: Matrix4::zeros(),
        };
        cam.calc_p_mat();
        cam.calc_v_mat();
        cam
    }
}
