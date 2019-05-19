extern crate log;
extern crate nalgebra as na;
extern crate simple_logger;

extern crate totality_events as events;
extern crate totality_gui as gui;
extern crate totality_io as io;
extern crate totality_model as geom;
extern crate totality_render as ren;
extern crate totality_sim as sim;
extern crate totality_sync as sync;
extern crate totality_threading as th;

mod link;
use link::*;

use e::{a, b, p, C, V};
use events::cb::ValueStore;
use events::hal as e;
use geom::{
    scene::{Scene},
    Model,
    geom::{VMat, FMat, Geom, tri::{TriMeshGeom, TriGeom}},
};
use gui::{
    base_components::{DisplayTextBox, Pane},
    Core as GUI,
};
use io::cb_arc;
use na::{Dynamic, Matrix, Matrix3, UnitQuaternion, U2, U3};
use ren::{Color, RenderReq, RenderSettings, TypedRenderStage};
use std::{
    env::{args, Args},
    option::Option,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};
use sync::triple_buffer as tb;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

#[derive(Debug, Copy, Clone)]
struct ConfigPaths<'a> {
    base_path: &'a str,
    // TODO add paths for specific subsystems -- avoid grouping everything into the same file
}

const DEFAULT_CONFIGURATION_PATHS: ConfigPaths = ConfigPaths {
    base_path: "./.tracer.cfg",
};

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
enum Action {
    Continue,
    Exit,
}

struct State {
    // r: std::Vec<disp::Renderer>, // think about this one a bit more
    ren: TypedRenderStage,
    sys: io::Manager, // TODO check mutability constraints
    sim: sim::Manager,
    gui: gui::Manager,
    c: Config,
    // shutdown flow
    shutdown: Arc<Mutex<io::CBFn>>,
    current_action: Arc<Mutex<Action>>,
    // graphics settings
    should_use_depth: Arc<Mutex<bool>>,
    settings_cb: Arc<Mutex<io::CBFn>>,
    should_restart_renderer: Arc<Mutex<bool>>,
    window_change_cb: Arc<Mutex<io::CBFn>>,
    // color flow
    color: Arc<Mutex<na::Vector4<f32>>>,
    color_changer: Arc<Mutex<io::CBFn>>,
    // fish selection
    fish: Arc<Mutex<i32>>,
    change_fish: Arc<Mutex<io::CBFn>>,
    // camera stuffs
    camera: Arc<Mutex<geom::camera::Camera>>,
    camera_mover: Arc<Mutex<io::CBFn>>,
    camera_roter: Arc<Mutex<io::CBFn>>,
}
impl State {
    fn new(cfg: Config) -> State {
        let sm = io::Manager::new();
        let c_tri0 = Arc::new(Box::new(TriGeom::new(
            na::Matrix3::new(0.5, 0., -0.5, -0.5, 0., -0.5, 0.5, 0., 0.5).transpose(),
            na::Vector3::new(2, 1, 0),
            vec![[0f32, 0f32], [1f32, 0f32], [0f32, 1f32]],
            Some("totality/res/thomas-veyrat-anglerfish-view01-3-4.jpg".to_string()),
        )) as Box<Geom>);
        let c_box = Arc::new(Box::new({
            TriMeshGeom::new(
                VMat::from_iterator(
                    8,
                    vec![
                        -0.5, -0.5, -0.5, -0.5, -0.5, 0.5, -0.5, 0.5, -0.5, -0.5, 0.5, 0.5, 0.5,
                        -0.5, -0.5, 0.5, -0.5, 0.5, 0.5, 0.5, -0.5, 0.5, 0.5, 0.5,
                    ]
                    .into_iter(),
                ),
                FMat::from_iterator(
                    12,
                    vec![
                        1, 4, 0, 5, 4, 1, // bottom
                        6, 3, 2, 7, 3, 6, // top
                        0, 2, 1, 3, 1, 2, // left
                        4, 7, 6, 5, 7, 4, // right
                        0, 6, 2, 6, 0, 4, // back
                        5, 3, 7, 3, 5, 1, // front
                    ]
                    .into_iter(),
                ),
                vec![
                    [0f32, 0f32],
                    [1f32, 0f32],
                    [0f32, 1f32],
                    [1f32, 1f32],
                    [0f32, 1f32],
                    [1f32, 1f32],
                    [0f32, 0f32],
                    [1f32, 0f32],
                ],
                Some("totality/res/53cb029b057a2dc4c753969a3ce83ff4.jpg".to_string()),
            )
        }) as Box<Geom>);
        info!("Constructed Triangle!");
        let mut box0_model = geom::Model::from_geom(c_box.clone());
        box0_model.set_omg(UnitQuaternion::from_axis_angle(
            &na::Vector3::y_axis(),
            -1.0,
        ));
        box0_model.set_scale(0.5);
        box0_model.set_pos(na::Vector3::new(0., 0.25, 0.));
        let mut box1_model = geom::Model::from_geom(c_box.clone());
        box1_model.set_omg(UnitQuaternion::from_axis_angle(&na::Vector3::y_axis(), 1.0));
        box1_model.set_scale(1.);
        box1_model.set_pos(na::Vector3::new(0., -0.5, 0.));
        let (arc_sc_s, rv_sc_d, ev_sc_d) = {
            let (s, d) = geom::scene::Scene::new(
                vec![c_tri0.clone(), c_box.clone()],
                vec![
                    geom::Model::from_geom(c_tri0.clone()),
                    box0_model,
                    box1_model,
                ],
            );
            let (rv, ev) = tb::buffer(d);
            (Arc::new(s), rv, ev)
        };
        // set up shutdown flow
        let c_act = Arc::new(Mutex::new(Action::Continue));
        let cb_shutdown = {
            let c_act = c_act.clone();
            cb_arc!("Exit", {
                debug!("What? You wanted to exit?");
                (*c_act.lock().unwrap()) = Action::Exit;
            })
        };
        sm.reg_imm(b::C::F(b::Flag::Close).into(), cb_shutdown.clone());
        sm.reg_imm(b::C::S(b::Key::Esc).into(), cb_shutdown.clone());
        let c_restart_render = Arc::new(Mutex::new(false));
        let cb_win_chg = {
            let c_restart_render = c_restart_render.clone();
            cb_arc!("Screen Resize", v, s, {
                if let V::P(p::V::ScreenSz(p::SzState(p))) = v {
                    let c = C::P(p::C::ScreenSz);
                    if let V::P(p::V::ScreenSz(p::SzState(p_prev))) = s.get(&c) {
                        if (p_prev - p).norm() > 1e-7 {
                            if let Ok(mut f) = c_restart_render.lock() {
                                (*f) = true
                            }
                        }
                    }
                }
            })
        };
        sm.reg_imm(p::C::ScreenSz.into(), cb_win_chg.clone());
        // set up settings flow
        let c_should_use_depth = Arc::new(Mutex::new(false));
        let cb_settings = {
            let c_should_use_depth = c_should_use_depth.clone();
            cb_arc!("Depth Usage Toggle", v, s, {
                if let V::B(b::V(_, b::State::UP)) = v {
                    if let Ok(mut f) = c_should_use_depth.lock() {
                        (*f) = !*f
                    }
                }
            })
        };
        sm.reg_imm(b::C::A('u').into(), cb_settings.clone());
        // set up render flow
        let c_fish = Arc::new(Mutex::new(1));
        let cb_change_fish = {
            let c_fish = c_fish.clone();
            cb_arc!("Fish Toggle", v, s, {
                if let V::B(b::V(_, b::State::UP)) = v {
                    if let Ok(mut f) = c_fish.lock() {
                        (*f) += 1;
                        (*f) %= 2;
                        info!("Fish {:?} needs to be rendered.", f);
                    }
                }
            })
        };
        sm.reg_imm(b::C::A('c').into(), cb_change_fish.clone());
        // set up color flow
        let c_color = Arc::new(Mutex::new(na::Vector4::new(1f32, 1f32, 1f32, 1f32)));
        let cb_color = {
            let c_color = c_color.clone();
            cb_arc!("Color from pos", v, s, {
                trace!("Mouse pos update fired with {:?}", v);
                let c = C::P(p::C::ScreenSz);
                if let V::P(p::V::CursorPos(p::PosState(p))) = v {
                    let e = s.get(&c);
                    if let V::P(p::V::ScreenSz(p::SzState(sz))) = e {
                        trace!(
                            "Current screen size: {:?}, Current cursor position: {:?}",
                            sz,
                            v
                        );
                        if let Ok(mut col) = c_color.lock() {
                            (*col) = na::Vector4::new(p[0] / sz[0], p[1] / sz[1], 1f32, 1f32);
                            trace!("Color applied: {:?}", col);
                        } else {
                            panic!("Mutex was poisoned! Can we really recover from this?");
                        }
                    } else {
                        panic!(
                            "The library is wrong. It gave me {:?} when requesting for {:?}.",
                            e, c
                        );
                    }
                } else {
                    panic!("I received an event ({:?}) I never signed up for....", v);
                }
            })
        };
        sm.reg_per(p::C::CursorPos.into(), cb_color.clone());
        let cam = Arc::new(Mutex::new(geom::camera::Camera::Perspective(
            geom::camera::PerspectiveCamera::default(),
        )));
        let cb_mover = {
            let cam = cam.clone();
            const MOVE_SPEED: f32 = 1.;
            cb_arc!("Mover", v, s, l_t, c_t, {
                let duration_held = *c_t - *l_t;
                trace!("Mover run on value: {:?}", v);
                let time_held = (duration_held.as_secs() as f64
                    + (duration_held.subsec_nanos() as f64 / 1_000_000_000f64))
                    as f32;
                if let Ok(mut cam) = cam.lock() {
                    if let V::B(b::V(b::C::A(c), b::State::DOWN)) = v {
                        match c {
                            'w' | 'a' | 's' | 'd' | 'q' | 'e' => {
                                (*cam).trans_cam_space(
                                    MOVE_SPEED
                                        * time_held
                                        * match c {
                                            'w' => na::Vector3::new(0., 0., -1.),
                                            'a' => na::Vector3::new(-1., 0., 0.),
                                            's' => na::Vector3::new(0., 0., 1.),
                                            'd' => na::Vector3::new(1., 0., 0.),
                                            'q' => na::Vector3::new(0., 1., 0.),
                                            'e' => na::Vector3::new(0., -1., 0.),
                                            _ => na::Vector3::new(0., 0., 0.),
                                        },
                                );
                                trace!("Camera at location: {:?}", cam.pos());
                            }
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
                let time_held = (duration_held.as_secs() as f64
                    + (duration_held.subsec_nanos() as f64 / 1_000_000_000f64))
                    as f32;
                if let Ok(mut cam) = cam.lock() {
                    (*cam).rot_cam_space(UnitQuaternion::from_axis_angle(
                        &na::Vector3::y_axis(),
                        ROT_SPEED * time_held,
                    ));
                }
            })
        };
        if let Ok(mut cam) = cam.lock() {
            (*cam).rot_cam_space(UnitQuaternion::from_axis_angle(
                &na::Vector3::y_axis(),
                std::f32::consts::FRAC_PI_4,
            ));
            (*cam).rot_cam_space(UnitQuaternion::from_axis_angle(
                &na::Vector3::x_axis(),
                -std::f32::consts::FRAC_PI_4,
            ));
            (*cam).trans_cam_space(na::Vector3::new(0., 0., 1.));
        }
        // sm.reg_per(b::C::A('n').into(), cb_rotor.clone());
        info!("Finished initial setup.");
        let sim_step = Duration::from_secs(1)
            .checked_div(120)
            .expect("Shouldn't be anything wrong here.");
        State {
            ren: TypedRenderStage::create(
                link::RenderData::new(
                    Some(rv_sc_d),
                    Some(arc_sc_s.clone()),
                    c_should_use_depth.clone(),
                    c_restart_render.clone(),
                    c_fish.clone(),
                    c_color.clone(),
                    cam.clone(),
                ),
                sm.win.clone(),
            ),
            sys: sm,
            sim: sim::Manager::new(
                sim_step,
                link::SimData::new(arc_sc_s, tb::Editing::EditingView(ev_sc_d)),
                vec![],
                vec![],
            )
            .expect("Could not create Simulation!"),
            gui: {
                // TODO build gui
                gui::Manager::new()
            },
            c: cfg,
            // shutdown flow
            shutdown: cb_shutdown,
            current_action: c_act,
            // graphics settings
            should_use_depth: c_should_use_depth,
            settings_cb: cb_settings,
            should_restart_renderer: c_restart_render,
            window_change_cb: cb_win_chg,
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
    fn step(&mut self) -> Action {
        // Every invocation
        // TODO update state (hot loops)
        // Every frame -- Vsync, and all the other fancy stuffs prohibit this from completely
        // working
        // TODO render
        // render(&mut self.r, &self.sc).expect("Nothing should be wrong yet...");
        self.gui.dispatch_draw();
        // self.ren.send_cmd(ren::RenderReq::Clear(ren::Color(na::Vector4::new(1., 1., 1., 1.)))); // NOTE this still works!
        // Every <variable> invocations
        // TODO run cold logic
        // possibly do above 2 steps in lock step
        // TODO query system state
        (*self.current_action.lock().unwrap()).clone()
    }
}
impl Drop for State {
    fn drop(&mut self) {
        info!("Shutting down all systems.");
    }
}

fn main() {
    simple_logger::init().unwrap();
    info!("Staring at {:?}.", std::path::Path::new(".").canonicalize());
    info!("Constructing + starting up.");
    let mut s = State::new(Config::new(DEFAULT_CONFIGURATION_PATHS, args()));
    info!("Beginning Loop!");
    let target_micros_per_frame = Duration::from_secs(1)
        .checked_div(120)
        .expect("Shouldn't be anything wrong here.");
    let mut last_frame = Instant::now();
    loop {
        let curr_frame = Instant::now();
        let time_step = curr_frame - last_frame;
        trace!("Frame begin. {:?} since last frame.", time_step);
        let act = s.step();
        if act == Action::Exit {
            break;
        }
        last_frame = curr_frame;
        let sim_duration = Instant::now() - curr_frame;
        trace!("Frame took {:?} to render.", sim_duration);
        if target_micros_per_frame > sim_duration {
            trace!("Sleeping for {:?}.", target_micros_per_frame - sim_duration);
            std::thread::sleep(target_micros_per_frame - sim_duration);
        }
    }
    info!("Beginning Cleanup!");
    drop(s);
    info!("And that's all for today, folks!")
}
