use std::borrow::Cow;

use na::Matrix4;
use vulkano::format::ClearColorValue;

pub struct RenderTask<'a> {
    pub cam: &'a model::camera::Camera,
    pub instancing_information: Vec<Cow<'a, model::AffineTransform>>,
    pub clear_color: ClearColorValue,
}

impl<'a> RenderTask<'a> {
    pub fn instancing_information_bytes(&self) -> Vec<Matrix4<f32>> {
        // TODO Figure out if I can do this better (cast vec of matrix to vec of bytes?)
        self.instancing_information.iter()
            .map(|transform| transform.mat())
            .collect()
    }
}
