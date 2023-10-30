use na::{Matrix4, UnitQuaternion, Vector3, Vector4};

#[derive(Debug, Copy, Clone)]
pub enum Camera {
    Perspective(PerspectiveCamera),
    Orthographic(OrthoCamera),
}
impl Camera {
    pub fn get_vp_mat(&self) -> Matrix4<f32> {
        match self {
            Camera::Perspective(cam) => cam.vp_mat(),
            Camera::Orthographic(cam) => cam.vp_mat(),
        }
    }
    pub fn trans_cam_space(&mut self, v: Vector3<f32>) {
        match self {
            Camera::Perspective(cam) => cam.trans_cam_space(v),
            Camera::Orthographic(cam) => cam.trans_cam_space(v),
        }
    }
    pub fn pos(&self) -> Vector3<f32> {
        match self {
            Camera::Perspective(cam) => cam.pos(),
            Camera::Orthographic(cam) => cam.pos(),
        }
    }
    pub fn rot_cam_space(&mut self, v: UnitQuaternion<f32>) {
        match self {
            Camera::Perspective(cam) => cam.rot_cam_space(v),
            Camera::Orthographic(_) => {}
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
    pub fn v_mat(&self) -> Matrix4<f32> {
        self._v_cache
    }
    pub fn p_mat(&self) -> Matrix4<f32> {
        self._p_cache
    }
    pub fn vp_mat(&self) -> Matrix4<f32> {
        self.p_mat() * self.v_mat()
    }
    fn calc_p_mat(&mut self) {
        self._p_cache.fill_with_identity();
        self._p_cache[(1, 1)] = -1.;
    }
    fn calc_v_mat(&mut self) {
        self._v_cache.fill_with_identity();
        self._v_cache[(2, 2)] = -1.;
        self._v_cache
            .fixed_slice_mut::<3, 1>(0, 3)
            .copy_from(&(self.position * -1.0));
    }
    pub fn trans(&mut self, shift: Vector3<f32>) {
        self.position += shift;
        self.calc_v_mat();
    }
    pub fn trans_cam_space(&mut self, shift: Vector3<f32>) {
        self.trans(self.orientation.inverse_transform_vector(&shift));
    }
    pub fn pos(&self) -> Vector3<f32> {
        self.position
    }
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
    pub fn v_mat(&self) -> Matrix4<f32> {
        self._v_cache
    }
    pub fn p_mat(&self) -> Matrix4<f32> {
        self._p_cache
    }
    pub fn vp_mat(&self) -> Matrix4<f32> {
        self.p_mat() * self.v_mat()
    }
    fn calc_p_mat(&mut self) {
        let n = self.near_plane_dist;
        let f = self.far_plane_dist;
        let a = -self.aspect;
        let cot = 1. / (self.fov * 0.5).tan();
        self._p_cache = Matrix4::new(
            cot / a,    0f32,              0f32,                 0f32,
               0f32,     cot,              0f32,                 0f32,
               0f32,    0f32, (f + n) / (n - f), 2. * f * n / (n - f),
               0f32,    0f32,             -1f32,                 0f32,
        );
        self._p_cache *= Matrix4::new(
            -1f32, 0f32, 0f32, 0f32,
            0f32, -1f32, 0f32, 0f32,
            0f32, 0f32, 1f32, 0f32,
            0f32, 0f32, 0f32, 1f32,
        );
    }
    fn calc_v_mat(&mut self) {
        self._v_cache = self.orientation.inverse().to_homogeneous() * Matrix4::new(
            1f32, 0f32, 0f32, -self.position[0],
            0f32, 1f32, 0f32, -self.position[1],
            0f32, 0f32, 1f32, -self.position[2],
            0f32, 0f32, 0f32, 1f32,
        );
    }
    pub fn rot(&mut self, rotor: UnitQuaternion<f32>) {
        self.orientation = rotor * self.orientation;
        self.calc_v_mat();
    }
    pub fn trans(&mut self, shift: Vector3<f32>) {
        self.position += shift;
        self.calc_v_mat();
    }
    pub fn trans_cam_space(&mut self, shift: Vector3<f32>) {
        self.trans(self.orientation.transform_vector(&shift));
    }
    pub fn rot_cam_space(&mut self, rotor: UnitQuaternion<f32>) {
        self.orientation = self.orientation * rotor;
        self.calc_v_mat();
    }
    pub fn pos(&self) -> Vector3<f32> {
        self.position
    }
}
impl Default for PerspectiveCamera {
    fn default() -> Self {
        let mut cam = PerspectiveCamera {
            near_plane_dist: 0.01f32,
            far_plane_dist: 1000.0f32,
            fov: std::f32::consts::FRAC_PI_2,
            aspect: 1.0f32,
            orientation: UnitQuaternion::identity(),
            position: Vector3::zeros(),
            _p_cache: Matrix4::zeros(),
            _v_cache: Matrix4::zeros(),
        };
        // The camera, by default, looks in the positive z direction, with positive y facing
        // upwards. However, we usually pretend we're looking "into" a scene, so we'll move
        // "forward" a little.
        cam.trans(Vector3::new(0.0, 0.0, 5.0));
        cam.calc_p_mat();
        cam.calc_v_mat();
        cam
    }
}
