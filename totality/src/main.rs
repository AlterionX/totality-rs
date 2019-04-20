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

use na::{Matrix3, U2};
use geom::{Model, scene::{Scene, TriGeom}};
use sys::{cb_arc, io::{self, e::{C, V, a, p, b}}, renderer::{BT, DT, IT, Color, TypedRenderReq, RenderReq, TypedRenderStage}};
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
    sc: Arc<RwLock<Option<Scene>>>,
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
    // color flow
    tri_m: Arc<Mutex<Model>>,
    tri_m_changer: Arc<Mutex<io::cb::CBFn>>,
}
impl State {
    fn new(cfg: Config) -> State {
        let mut sm = io::Manager::new();
        let c_tri = Arc::new(Box::new(geom::scene::TriGeom::new(
             na::Matrix3::new(
                 0.5,  0.5,  0f32,
                -0.5,  0.5,  0f32,
                 0f32, 0f32, 0f32,
            ),
            na::Vector3::new(0u32, 1, 2),
        )) as Box<geom::Geom>);
        let sc = Arc::new(RwLock::new(Some(geom::scene::Scene::new(
            vec![c_tri.clone()],
            vec![geom::Model::from_geom(c_tri.clone())]
        ))));
        let renderer = Option::Some(TypedRenderStage::create(sc.clone(), sm.win.clone()));
        // set up shutdown flow
        let c_act = Arc::new(Mutex::new(Action::Continue));
        let cb_shutdown = {
            let c_act = c_act.clone();
            cb_arc!("Exit", { debug!("What? You wanted to exit?"); (*c_act.lock().unwrap()) = Action::Exit; })
        };
        sm.reg_imm(b::C::F(b::Flag::Close).into(), cb_shutdown.clone());
        sm.reg_imm(b::C::A('c').into(), cb_shutdown.clone());
        sm.reg_imm(b::C::S(b::Key::Esc).into(), cb_shutdown.clone());
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
        let c_tri_m = Arc::new(Mutex::new(geom::Model::from_geom(c_tri.clone())));
        let cb_tri_m = {
            let c_tri_m = c_tri_m.clone();
            cb_arc!("TriPos", v, s, {
                let c = C::P(p::C::ScreenSz);
                if let V::P(p::V::CursorPos(p::PosState(p))) = v {
                    let e = s.get(&c);
                    if let V::P(p::V::ScreenSz(p::SzState(sz))) = e {
                        trace!("Current screen size: {:?}, Current cursor position: {:?}", sz, v);
                        if let Ok(mut tri_m) = c_tri_m.lock() {
                            let div = p.component_div(&sz);
                            let mut p = na::Vector3::new(div[0], div[1], 0f32);
                            p *= 2f32;
                            p -= na::Vector3::new(1f32, 1f32, 0f32);
                            (*tri_m).set_off_v(&p);
                            trace!("Triangle position changed to: {:?}", v);
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
        sm.reg_per(io::e::p::C::CursorPos.into(), cb_tri_m.clone());
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
            // tri pos
            tri_m: c_tri_m,
            tri_m_changer: cb_tri_m,
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
        if let Some(ref rs) = self.rs {
            rs.send_cmd(RenderReq::Draw::<BT, DT, IT>(
                    (*self.tri_m.lock().expect("The mutex for the triangle is poisoned.")).clone(),
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
    // simple_logger::init().unwrap();
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

