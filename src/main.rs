extern crate nalgebra as na;
extern crate image as img;
extern crate winit;
extern crate simple_logger;
extern crate log;
extern crate arrayvec as av;

mod sys;
mod geom;

use std::{
    option::Option,
    env::{args, Args},
    result::Result,
    string::String,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant}
};

use geom::Scene;
use sys::{io, renderer::{Color, RenderStage, RenderReq}};
#[allow(dead_code)]
use log::{debug, warn, error, info, trace};

fn save(img: geom::Img) {
    let rows = img.len() as u32;
    let cols = img[0].len() as u32;
    let mut buf = img::ImageBuffer::new(cols, rows);
    for row in 0u32..rows {
        for col in 0u32..cols {
            let color = img[row as usize][col as usize];
            buf.put_pixel(
                col, row,
                img::Rgb([color[0], color[1], color[2]])
            );
        }
    }
    buf.save("something.png").unwrap();
}

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
    rs: Option<RenderStage>,
    sys: Option<io::Manager>, // TODO check mutability constraints
    c: Config,
    // shutdown flow
    shutdown: Arc<Mutex<io::cb::CBFn>>,
    current_action: Arc<Mutex<Action>>,
    // color flow
    color: Arc<Mutex<na::Vector4<f32>>>,
    color_changer: Arc<Mutex<io::cb::CBFn>>,
}
impl State {
    fn new(cfg: Config) -> State {
        let mut sm = io::Manager::new();
        let sc = Arc::new(RwLock::new(std::option::Option::None));
        let renderer = Option::Some(RenderStage::new(sc.clone(), sm.win.clone()));
        // set up shutdown flow
        let c_act = Arc::new(Mutex::new(Action::Continue));
        let internal_c_act = c_act.clone();
        let cb_shutdown = Arc::new(Mutex::new(move |_: &io::e::State, _: &io::e::V| { debug!("What? You wanted to exit?"); (*internal_c_act.lock().unwrap()) = Action::Exit; }));
        sm.reg_imm(io::e::b::C::F(io::e::b::Flag::Close).into(), cb_shutdown.clone());
        // set up color flow
        let c_color = Arc::new(Mutex::new(na::Vector4::new(1f32,1f32,1f32,1f32)));
        let internal_c_color = c_color.clone();
        let cb_color = Arc::new(Mutex::new(move |_: &io::e::State, v: &io::e::V| {
            match v {
                io::e::V::P(io::e::p::V::MousePos(io::e::p::PosState(v))) => (*internal_c_color.lock().unwrap()) = na::Vector4::new(v[0], v[1], 1f32, 1f32),
                _ => (),
            }
        }));
        sm.reg_per(io::e::p::C::MousePos.into(), cb_color.clone());
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
            rs.send_cmd(RenderReq::Clear(Color(na::Vector4::new(
                original_color[0] as f32,
                original_color[1] as f32,
                original_color[2] as f32,
                original_color[3] as f32,
            )))).expect("No problems expected.");
        }
        // Every <variable> invocations
        // TODO run cold logic
        // possibly do above 2 steps in lock step
        // TODO query system state
        (*self.current_action.lock().unwrap()).clone()
    }
    fn cleanup(mut self) -> Result<(), String> {
        let mut res = Result::Err(String::from("Could not clean up State."));
        // TODO change to let chaining once available
        info!("Shutting down system management.");
        if let Option::Some(sys) = self.sys.take() {
            res = std::result::Result::Ok(sys.finish().expect("Could not complete `Manager` finish."))
        }
        info!("Shutting down rendering systems.");
        if let Some(rs) = self.rs.take() {
            res = std::result::Result::Ok(rs.finish().expect("Could not complete `Manager` finish."))
        }
        res
    }
}
impl Drop for State {
    fn drop(&mut self) {
        assert!(self.sys.is_none(), "You MUST call either cleanup on `State` to clean it up.");
    }
}

fn main() {
    simple_logger::init().unwrap();
    info!("Constructing + starting up.");
    let mut s = State::new(Config::new(DEFAULT_CONFIGURATION_PATHS, args()));
    info!("Beginning Loop!");
    let target_micros_per_frame = Duration::from_secs(1).checked_div(120).expect("Shouldn't be anything wrong here.");
    let mut last_frame = Instant::now();
    loop {
        let curr_frame = Instant::now();
        let time_step = curr_frame - last_frame;
        info!("Frame begin. {:?} since last frame.", time_step);
        let act = s.step(time_step);
        if act == Action::Exit { break }
        last_frame = curr_frame;
        let sim_duration = Instant::now() - curr_frame;
        info!("Frame took {:?} to render.", sim_duration);
        if target_micros_per_frame > sim_duration {
            info!("Sleeping for {:?}.", target_micros_per_frame - sim_duration);
            std::thread::sleep(target_micros_per_frame - sim_duration);
        }
    };
    info!("Beginning Cleanup!");
    s.cleanup().expect("State cleanup failed.");
    log::info!("And that's all for today, folks!")
}

