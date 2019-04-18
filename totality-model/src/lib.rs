extern crate nalgebra as na;

use std::sync::Arc;
use na::{
    Vector3,
    UnitQuaternion,
    Vector2,
    Matrix3,
};

pub struct Camera {
    loc: Vector3<f32>, // loc in world
    rot: UnitQuaternion<f32>, // ori in world
    dim: Vector2<f32>, // size in world units
    foc: f32, // focal distance, world units
    res: Vector2<u32>, // pixels
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            loc: Vector3::zeros(),
            rot: UnitQuaternion::identity(),
            dim: Vector2::new(2., 2.),
            foc: 0.025,
            res: Vector2::new(4u32, 4u32),
        }
    }
}

impl Camera {
    pub fn take_img(&self, s: &Scene) -> Img {
        println!("Hello");
        let mut img = Vec::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let rrr = self.get_rays();
        for (row, rr) in rrr.iter().enumerate() {
            img.push(Vec::new());
            let mut pix_row = match img.last_mut() {
                None => panic!("Welp, i just put a Vec into another Vec and now I can't use it anymore. This shouldn't be possible."),
                Some(v) => v,
            };
            for (col, r) in rr.iter().enumerate() {
                pix_row.push(Vector3::new(0u8, 0u8, 0u8));
                let ray = (*r).clone();
                let th_tx = tx.clone();
                let s = Arc::new(Scene::default());
                let k = Box::new(move || {
                    th_tx.send((row, col, (*s).intersect(&ray))).unwrap()
                });
            }
        }
        println!("Waiting for results");
        for i in 0..(self.res[0] * self.res[1]) {
            let (row, col, color) = rx.recv().unwrap();
            println!("Result {} arrived.", i);
            img[row][col] = color;
        }
        println!("Goodbye");
        img
    }
    fn get_rays(&self) -> Vec<Vec<Ray>> {
        let mut depo = Vec::new();
        let cell_sz = {
            let res: Vector2<f32> = na::convert(self.res);
            self.dim.component_div(&res)
        };
        let mut pix = Vector2::zeros();
        for row in 0u32..self.res.y {
            pix[0] = row;
            let mut row_vec = Vec::new();
            depo.push(row_vec);
            let row_vec = match depo.last_mut() {
                None => panic!("Welp, i just put a Vec into another Vec and now I can't use it anymore. This shouldn't be possible."),
                Some(v) => v,
            };
            for col in 0u32..self.res.x {
                pix[1] = col;
                (*row_vec).push(self.get_ray(&pix, &cell_sz));
            }
        }
        depo
    }
    fn get_ray(&self, pix: &Vector2<u32>, sz: &Vector2<f32>) -> Ray {
        let cam_pix_loc = sz.component_mul(&na::convert::<Vector2<u32>, Vector2<f32>>(*pix)) + (0.5f32 * sz) - (0.5f32 * self.dim);
        let center = self.rot.to_rotation_matrix() * (self.loc + Vector3::new(cam_pix_loc[0], cam_pix_loc[1], -self.foc));
        Ray {
            src: self.loc,
            dir: na::normalize(&center),
            t: f32::sqrt(cam_pix_loc[0] * cam_pix_loc[0] + cam_pix_loc[1] * cam_pix_loc[1] + self.foc * self.foc),
            col: Vector3::new(1u8, 1u8, 1u8),
        }
    }
}

pub struct Scene {
    mm: Vec<Mesh>, // roots for scene graph
    ll: Vec<Box<Light>>, // lights in scene
}

unsafe impl Sync for Scene {}
unsafe impl Send for Scene {}
impl Default for Scene {
    fn default() -> Scene {
        Scene {
            mm: Vec::new(),
            ll: Vec::new(),
        }
    }
}
impl Scene {
    pub fn new(mm: Vec<Mesh>, ll: Vec<Box<Light>>) -> Scene {
        Scene{ mm, ll }
    }
    fn draw(&self, c: Camera) -> Img {
        let mut pixels = Vec::new();
        for (row, rr) in c.get_rays().iter().enumerate() {
            pixels.push(Vec::new());
            for (col, r) in rr.iter().enumerate() {
                pixels[row].push(self.intersect(r));
            }
        }
        Vec::new()
    }
    fn intersect(&self, r: &Ray) -> Color {
        let mut isects: Vec<Isect> = Vec::new();
        for m in self.mm.iter() {
            isects.extend(m.intersect(r));
        }
        let mut isects = vec![];
        isects.sort_by(|a: &Isect, b: &Isect| { a.r.t.partial_cmp(&b.r.t).unwrap() });
        let c = Vector3::zeros();
        if isects.len() > 0 {
        }
        c
    }
    fn process(&self, isect: Isect) -> Vec<Ray> {
        // generate all rays
        Vec::new()
    }
}

pub trait Light {
    fn color(&self) -> ();
}

pub struct Isect {
    r: Ray,
    m: Mesh,
    n: Vector3<f32>, // normal
    mat: Material,
}

impl Isect {
    fn color(&self, r: &Ray, ll: &Vec<Box<dyn Light>>) -> Color {
        let c = Vector3::zeros();
        for l in ll.iter() {
            // if l.is_visible_to(self.r.get_loc()) {
            //     c += mat.get_color(l);
            // }
        }
        c
    }
}

pub struct Mesh {
    pp: Vec<Vector3<f32>>,
    tt: Vec<Triangle>,
    trans: Matrix3<f32>,
    rot: Matrix3<f32>
}

impl Mesh {
    fn intersect(&self, r: &Ray) -> Vec<Isect> {
        let v = Vec::new();
        // do stuff
        v
    }
}

pub struct Triangle {
    vv: [u32; 3],
    mat: [Material; 3]
}

pub struct Material {
    amb: Vector3<f32>,
    dif: Vector3<f32>,
    spe: Vector3<f32>,
}

impl Material {
    fn get_color(&self, l: &Light) {
        // light
    }
}

#[derive(Clone)]
pub struct Ray {
    src: Vector3<f32>,
    dir: Vector3<f32>,
    t: f32,
    col: Color,
}

impl Ray {
    pub fn get_loc(&self) -> Vector3<f32> {
        self.src + self.t * self.dir
    }
}

pub type Color = Vector3<u8>;

pub type Img = Vec<Vec<Color>>;
