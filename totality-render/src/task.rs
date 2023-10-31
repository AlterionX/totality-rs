use std::borrow::Cow;

use na::Matrix4;
use vulkano::format::ClearColorValue;

use model::geom::tri::TriMeshGeom;

#[derive(Debug, Clone)]
pub struct RenderTask<'a> {
    pub draw_wireframe: bool,
    pub cam: &'a model::camera::Camera,
    pub draws: Vec<DrawTask<'a>>,
    pub clear_color: ClearColorValue,
}

#[derive(Debug, Clone)]
pub struct DrawTask<'a> {
    pub mesh: Cow<'a, TriMeshGeom>,
    pub instancing_information: Vec<Cow<'a, model::AffineTransform>>,
}

impl<'a> RenderTask<'a> {
    pub fn instancing_information_bytes(&self) -> Vec<Matrix4<f32>> {
        // TODO Figure out if I can do this better.
        self.draws.iter()
            .flat_map(|draw| {
                draw.instancing_information.iter()
                .map(|transform| transform.mat())
            })
            .collect()
    }
}
