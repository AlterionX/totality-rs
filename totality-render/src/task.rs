use std::borrow::Cow;

use na::{Matrix4, Vector3, UnitVector3};
use vulkano::format::ClearColorValue;

use model::geom::tri::TriMeshGeom;

#[derive(Debug, Clone)]
pub struct RenderTask<'a> {
    pub draw_wireframe: bool,
    pub cam: &'a model::camera::Camera,
    pub draws: Vec<DrawTask<'a>>,
    pub clear_color: ClearColorValue,
    pub lights: LightCollection,
}

#[derive(Debug, Clone)]
pub struct LightCollection(pub Vec<Light>);

impl LightCollection {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = vec![0u8; self.0.len() * Light::bytes()];
        for (idx, light) in self.0.iter().enumerate() {
            let start = idx * Light::bytes();
            let end = start + Light::bytes();
            light.write_as_bytes_to(&mut buffer[start..end]);
        }
        buffer
    }
}

#[derive(Debug, Clone)]
pub enum Light {
    Directional(DirectionalLight),
    Point(PointLight),
}

impl Light {
    fn bytes() -> usize {
        32
    }

    // TODO Figure out how to do this properly.
    fn write_as_bytes_to(&self, buffer: &mut [u8]) {
        match self {
            Self::Point(plight) => {
                buffer[0..12].copy_from_slice(bytemuck::cast_slice(plight.color.as_slice()));
                buffer[12..16].copy_from_slice(bytemuck::cast_slice(PointLight::identifier().as_slice()));
                buffer[16..28].copy_from_slice(bytemuck::cast_slice(plight.position.as_slice()));
                buffer[28..32].copy_from_slice(&[0, 0, 0, 0]);
            },
            Self::Directional(dlight) => {
                buffer[0..12].copy_from_slice(bytemuck::cast_slice(dlight.color.as_slice()));
                buffer[12..16].copy_from_slice(bytemuck::cast_slice(DirectionalLight::identifier().as_slice()));
                buffer[16..28].copy_from_slice(bytemuck::cast_slice(dlight.direction.as_slice()));
                buffer[28..32].copy_from_slice(&[0, 0, 0, 0]);
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PointLight {
    pub color: Vector3<f32>,
    pub position: Vector3<f32>,
}

impl PointLight {
    fn identifier() -> [f32; 1] {
        [1.]
    }
}

#[derive(Debug, Clone)]
pub struct DirectionalLight {
    pub color: Vector3<f32>,
    pub direction: UnitVector3<f32>,
}

impl DirectionalLight {
    fn identifier() -> [f32; 1] {
        [2.]
    }
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
