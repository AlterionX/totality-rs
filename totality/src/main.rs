extern crate nalgebra as na;
extern crate image as img;
extern crate winit;
extern crate simple_logger;
extern crate log;
extern crate arrayvec as av;
extern crate totality_sys as sys;
extern crate totality_threading as th;
extern crate totality_model as geom;

use std::{
    option::Option,
    env::{args, Args},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant}
};

use na::{Matrix, Matrix3, U2, U3, Dynamic, UnitQuaternion};
use geom::{Model, scene::{Scene, TriGeom}};
use sys::{cb_arc, io::{self, e::{C, V, a, p, b}}, renderer::{IT, Color, TypedRenderReq, RenderReq, TypedRenderStage}};
#[allow(dead_code)]
use log::{debug, warn, error, info, trace};

#[derive(Debug, Copy, Clone)]
struct ConfigPaths<'a> {
    base_path: &'a str,
    // TODO add paths for specific subsystems -- avoid grouping everything into the same file
}

const DEFAULT_CONFIGURATION_PATHS: ConfigPaths = ConfigPaths { base_path: "./.tracer.cfg" };

#[derive(Debug, Copy, Clone)]
struct Config {
    // TODO add things that need to go here (typically)
}
impl Config {
    fn new(paths: ConfigPaths, args: Args) -> Config {
        let used_paths = DEFAULT_CONFIGURATION_PATHS.clone();
        // TODO overwrite path that flags specify
        let c = Config {/*TODO load*/};
        // TODO proceed to load coarse selection options
        // TODO overwrite options if more flags exist
        return c;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Action { Continue, Exit, }

struct State {
    sc: Arc<Option<RwLock<Scene>>>,
    // r: std::Vec<disp::Renderer>, // think about this one a bit more
    rs: Option<TypedRenderStage>,
    sys: Option<io::Manager>, // TODO check mutability constraints
    c: Config,
    // shutdown flow
    shutdown: Arc<Mutex<io::cb::CBFn>>,
    current_action: Arc<Mutex<Action>>,
    // color flow
    color: Arc<Mutex<na::Vector4<f32>>>,
    color_changer: Arc<Mutex<io::cb::CBFn>>,
    // fish selection
    fish: Arc<Mutex<i32>>,
    change_fish: Arc<Mutex<io::cb::CBFn>>,
    // camera stuffs
    camera: Arc<Mutex<geom::camera::Camera>>,
    camera_mover: Arc<Mutex<io::cb::CBFn>>,
    camera_roter: Arc<Mutex<io::cb::CBFn>>,
}
impl State {
    fn new(cfg: Config) -> State {
        let mut sm = io::Manager::new();
        let c_tri0 = Arc::new(Box::new(geom::scene::TriGeom::new(
             na::Matrix3::new(
                 0.5,  0.5,  -1f32,
                -0.5,  0.5,  -1f32,
                 0f32, 0f32, -1f32,
            ),
            na::Vector3::new(2u32, 1, 0),
            vec![[0f32, 0f32], [1f32, 0f32], [0f32, 1f32]],
            Some("totality/res/thomas-veyrat-anglerfish-view01-3-4.jpg".to_string()),
        )) as Box<geom::Geom>);
        let c_tri1 = Arc::new(Box::new({
            geom::scene::TriMeshGeom::new(
                geom::VMat::from_iterator(8, vec![
                    -0.5, -0.5, -0.5,
                    -0.5, -0.5,  0.5,
                    -0.5,  0.5, -0.5,
                    -0.5,  0.5,  0.5,
                     0.5, -0.5, -0.5,
                     0.5, -0.5,  0.5,
                     0.5,  0.5, -0.5,
                     0.5,  0.5,  0.5,
                ].into_iter()),
                geom::FMat::from_iterator(12, vec![
                    1, 4, 0, 5, 4, 1, // bottom
                    6, 3, 2, 7, 3, 6, // top
                    0, 2, 1, 3, 1, 2, // left
                    4, 7, 6, 5, 7, 4, // right
                    0, 6, 2, 6, 0, 4, // back
                    5, 3, 7, 3, 5, 1, // front
                ].into_iter()),
                vec![
                    [0f32, 0f32], [1f32, 0f32],
                    [0f32, 1f32], [1f32, 1f32],
                    [0f32, 1f32], [1f32, 1f32],
                    [0f32, 0f32], [1f32, 0f32],
                ],
                Some("totality/res/53cb029b057a2dc4c753969a3ce83ff4.jpg".to_string()),
            )
        }) as Box<geom::Geom>);
        info!("Constructed Triangle!");
        let sc = Arc::new(Some(RwLock::new(geom::scene::Scene::new(
            vec![c_tri0.clone(), c_tri1.clone()],
            vec![geom::Model::from_geom(c_tri0.clone()), geom::Model::from_geom(c_tri1.clone())]
        ))));
        let renderer = Option::Some(TypedRenderStage::create(sc.clone(), sm.win.clone()));
        // set up shutdown flow
        let c_act = Arc::new(Mutex::new(Action::Continue));
        let cb_shutdown = {
            let c_act = c_act.clone();
            cb_arc!("Exit", { debug!("What? You wanted to exit?"); (*c_act.lock().unwrap()) = Action::Exit; })
        };
        sm.reg_imm(b::C::F(b::Flag::Close).into(), cb_shutdown.clone());
        sm.reg_imm(b::C::S(b::Key::Esc).into(), cb_shutdown.clone());
        let c_fish = Arc::new(Mutex::new(0));
        let cb_change_fish = {
            let c_fish = c_fish.clone();
            cb_arc!("Fish Toggle", v, s, {
                if let V::B(b::V(_, b::State::UP)) = v {
                    if let Ok(mut f) = c_fish.lock() { (*f) += 1; (*f) %= 2; info!("Fish {:?} needs to be rendered.", f); }
                }
            })
        };
        sm.reg_imm(b::C::A('c').into(), cb_change_fish.clone());
        // set up color flow
        let c_color = Arc::new(Mutex::new(na::Vector4::new(1f32,1f32,1f32,1f32)));
        let cb_color = {
            let c_color = c_color.clone();
            cb_arc!("Color from pos", v, s, {
                trace!("Mouse pos update fired with {:?}", v);
                let c = C::P(p::C::ScreenSz);
                if let V::P(p::V::CursorPos(p::PosState(p))) = v {
                    let e = s.get(&c);
                    if let V::P(p::V::ScreenSz(p::SzState(sz))) = e {
                        trace!("Current screen size: {:?}, Current cursor position: {:?}", sz, v);
                        if let Ok(mut col) = c_color.lock() {
                            (*col) = na::Vector4::new(p[0] / sz[0], p[1] / sz[1], 1f32, 1f32);
                            trace!("Color applied: {:?}", col);
                        } else {
                            panic!("Mutex was poisoned! Can we really recover from this?");
                        }
                    } else {
                        panic!("The library is wrong. It gave me {:?} when requesting for {:?}.", e, c);
                    }
                } else {
                    panic!("I received an event I never signed up for....");
                }
            })
        };
        sm.reg_per(io::e::p::C::CursorPos.into(), cb_color.clone());
        let cam = Arc::new(Mutex::new(geom::camera::Camera::Orthographic(geom::camera::OrthoCamera::default())));
        let cam = Arc::new(Mutex::new(geom::camera::Camera::Perspective(geom::camera::PerspectiveCamera::default())));
        let cb_mover = {
            let cam = cam.clone();
            const MOVE_SPEED: f32 = 1.;
            cb_arc!("Mover", v, s, l_t, c_t, {
                let duration_held = *c_t - *l_t;
                trace!("Mover run on value: {:?}", v);
                let time_held = (duration_held.as_secs() as f64 + (duration_held.subsec_nanos() as f64 / 1_000_000_000f64)) as f32;
                if let Ok(mut cam) = cam.lock() {
                    if let V::B(b::V(b::C::A(c), b::State::DOWN)) = v {
                        match c {
                            'w' | 'a' | 's' | 'd' | 'q' | 'e' => {
                                (*cam).trans_cam_space(MOVE_SPEED * time_held * match c {
                                    'w' => na::Vector3::new(0., 0., -1.),
                                    'a' => na::Vector3::new(-1., 0., 0.),
                                    's' => na::Vector3::new(0., 0., 1.),
                                    'd' => na::Vector3::new(1., 0., 0.),
                                    'q' => na::Vector3::new(0., 1., 0.),
                                    'e' => na::Vector3::new(0., -1., 0.),
                                    _ => na::Vector3::new(0., 0., 0.),
                                });
                                trace!("Camera at location: {:?}", cam.pos());
                            },
                            _ => (),
                        }
                    }
                }
            })
        };
        sm.reg_per(b::C::A('w').into(), cb_mover.clone());
        sm.reg_per(b::C::A('a').into(), cb_mover.clone());
        sm.reg_per(b::C::A('s').into(), cb_mover.clone());
        sm.reg_per(b::C::A('d').into(), cb_mover.clone());
        sm.reg_per(b::C::A('q').into(), cb_mover.clone());
        sm.reg_per(b::C::A('e').into(), cb_mover.clone());
        let cb_rotor = {
            let cam = cam.clone();
            const ROT_SPEED: f32 = -1.0;
            cb_arc!("Rotor", v, s, l_t, c_t, {
                let duration_held = *c_t - *l_t;
                trace!("Rotor run.");
                let time_held = (duration_held.as_secs() as f64 + (duration_held.subsec_nanos() as f64 / 1_000_000_000f64)) as f32;
                if let Ok(mut cam) = cam.lock() {
                    (*cam).rot_cam_space(UnitQuaternion::from_axis_angle(&na::Vector3::y_axis(), ROT_SPEED * time_held));
                }
            })
        };
        if let Ok(mut cam) = cam.lock() {
            (*cam).rot_cam_space(UnitQuaternion::from_axis_angle(&na::Vector3::y_axis(), std::f32::consts::FRAC_PI_4));
            (*cam).rot_cam_space(UnitQuaternion::from_axis_angle(&na::Vector3::x_axis(), std::f32::consts::FRAC_PI_4));
            // (*cam).trans_cam_space(na::Vector3::new(0., 0., 1.));
            if let geom::camera::Camera::Perspective(mut p) = *cam {
                info!("Camers: {:?}", p);
            }
        }
        // sm.reg_per(b::C::A('n').into(), cb_rotor.clone());
        info!("Finished initial setup.");
        State {
            sc: sc,
            rs: renderer,
            sys: Option::Some(sm),
            c: cfg,
            // shutdown flow
            shutdown: cb_shutdown,
            current_action: c_act,
            // color flow
            color: c_color,
            color_changer: cb_color,
            // fish selection
            fish: c_fish,
            change_fish: cb_change_fish,
            // camera
            camera: cam,
            camera_mover: cb_mover,
            camera_roter: cb_rotor,
        }
    }
    fn step(&mut self, delta: Duration) -> Action {
        // Every invocation
        // TODO update state (hot loops)
        // Every frame -- Vsync, and all the other fancy stuffs prohibit this from completely
        // working
        // TODO render
        // render(&mut self.r, &self.sc).expect("Nothing should be wrong yet...");
        let original_color = self.color.lock().expect("Seriously?");
        let fish_id = self.fish.lock().expect("Seriously?");
        let cam_clone = (*if let Ok(ref cam) = self.camera.lock() { cam } else { panic!("Camera poisoned!") }).clone();
        let rs_ref = if let Some(ref rs) = self.rs { rs } else { return Action::Continue; };
        if let Some(ref sc_lk) = *self.sc {
            let model_clone = if let Ok(sc) = sc_lk.read() {
                sc.dynamics.mm[*fish_id as usize].clone()
            } else { panic!("The scene mutex is poisoned.") };
            rs_ref.send_cmd(RenderReq::Draw(
                model_clone, cam_clone,
                Color(na::Vector4::new(
                        original_color[0] as f32,
                        original_color[1] as f32,
                        original_color[2] as f32,
                        original_color[3] as f32,
                ))
            )).expect("No problems expected.");
        }
        // Every <variable> invocations
        // TODO run cold logic
        // possibly do above 2 steps in lock step
        // TODO query system state
        (*self.current_action.lock().unwrap()).clone()
    }
    fn cleanup(mut self) -> (Option<sys::io::FinishResult>, Option<sys::renderer::FinishResult>) {
        // TODO change to let chaining once available
        ({
            info!("Shutting down system management.");
            self.sys.take().map(|s| s.finish())
        }, {
            info!("Shutting down rendering systems.");
            self.rs.take().map(|r| r.finish())
        })
    }
}
impl Drop for State {
    fn drop(&mut self) {
        assert!(self.sys.is_none(), "You MUST call either cleanup on `State` to clean it up.");
    }
}

fn main() {
    simple_logger::init().unwrap();
    info!("Staring at {:?}.", std::path::Path::new(".").canonicalize());
    info!("Constructing + starting up.");
    let mut s = State::new(Config::new(DEFAULT_CONFIGURATION_PATHS, args()));
    info!("Beginning Loop!");
    let target_micros_per_frame = Duration::from_secs(1).checked_div(120).expect("Shouldn't be anything wrong here.");
    let mut last_frame = Instant::now();
    loop {
        let curr_frame = Instant::now();
        let time_step = curr_frame - last_frame;
        trace!("Frame begin. {:?} since last frame.", time_step);
        let act = s.step(time_step);
        if act == Action::Exit { break }
        last_frame = curr_frame;
        let sim_duration = Instant::now() - curr_frame;
        trace!("Frame took {:?} to render.", sim_duration);
        if target_micros_per_frame > sim_duration {
            trace!("Sleeping for {:?}.", target_micros_per_frame - sim_duration);
            std::thread::sleep(target_micros_per_frame - sim_duration);
        }
    };
    info!("Beginning Cleanup!");
    match s.cleanup() {
        _ => ()
    }
    info!("And that's all for today, folks!")
}

