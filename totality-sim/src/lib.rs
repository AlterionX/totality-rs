extern crate totality_threading as th;
extern crate totality_model as geom;
extern crate log;
extern crate nalgebra as na;

use std::{
    sync::{Arc, Mutex, Weak, RwLock},
    time::{Duration},
    ops::DerefMut,
};
use na::UnitQuaternion;

#[allow(dead_code)]
use log::{trace, debug, info, warn, error};

pub trait PhysicsHook: FnMut(&geom::scene::Static, &mut geom::scene::Dynamic) + Send + 'static {}
impl <F: FnMut(&geom::scene::Static, &mut geom::scene::Dynamic) + Send + 'static> PhysicsHook for F {}

struct Simulation {
    scene: Arc<RwLock<Option<geom::scene::Scene>>>,
    post_cbs: Vec<Box<PhysicsHook>>,
    pre_cbs: Vec<Box<PhysicsHook>>,
    time_step: Duration,
}
unsafe impl Send for Simulation {}
impl Simulation {
    fn dur_as_f64(d: &Duration) -> f64 {
        (d.as_secs() as f64) + (d.subsec_nanos() as f64) / 1_000_000_000f64
    }
    pub fn step(&mut self) {
        let step = Self::dur_as_f64(&self.time_step) as f32;
        trace!("Step: {:?}", step);
        // call pre
        // simulate
        let opt = if let Ok(k) = self.scene.read() {
            if let Some(ref sc) = *k {
                Some(sc.advance())
            } else { None }
        } else { panic!("Scene corrupted!") };
        if let Some((r, w)) = opt {
            // actually does exist, so lock and update
            if let (Ok(r), Ok(mut w)) = (r.read(), w.write()) {
                for (r_ele, w_ele) in (*r).mm.iter().zip((w.deref_mut()).mm.iter_mut()) {
                    w_ele.set_state(
                        r_ele.pos + r_ele.vel * step,
                        r_ele.vel,
                        r_ele.ori * UnitQuaternion::identity().nlerp(&r_ele.omg, step),
                        r_ele.omg,
                        r_ele.scale,
                    );
                };
            };
        };
        // call post
    }
    pub fn as_thread(d: Duration, sc: Arc<RwLock<Option<geom::scene::Scene>>>, post_cbs: Vec<Box<PhysicsHook>>, pre_cbs: Vec<Box<PhysicsHook>>) -> Result<th::killable_thread::KillableThread<()>, std::io::Error> {
        th::create_duration_kt!((), d, "Simulation", {
            let mut sim = {
                Simulation {
                    scene: sc.clone(),
                    post_cbs: post_cbs,
                    pre_cbs: pre_cbs,
                    time_step: d,
                }
            }
        }, {
            // for pre in sim.pre_cbs {
            //     pre(sim.mutated.statics, sim.mutated.dynamics);
            // }
            sim.step();
            // for post in sim.post_cbs {
            //     post(sc);
            // }
        }, {})
    }
}

pub type FinishResult = th::killable_thread::FinishResult<()>;

pub struct SimulationManager {
    sim_th: Option<th::killable_thread::KillableThread<()>>,
}
impl SimulationManager {
    pub fn new(d: Duration, sc: Arc<RwLock<Option<geom::scene::Scene>>>, post_cbs: Vec<Box<PhysicsHook>>, pre_cbs: Vec<Box<PhysicsHook>>) -> Result<SimulationManager, std::io::Error> {
        Ok(SimulationManager {
            sim_th: Some(Simulation::as_thread(d, sc, post_cbs, pre_cbs)?)
        })
    }
    pub fn finish(mut self) -> FinishResult {
        self.sim_th.take().map_or_else(|| Option::None, |kt| kt.finish())
    }
}
impl Drop for SimulationManager {
    fn drop(&mut self) {
        if self.sim_th.is_some() {
            panic!("Must call finish on SimulationManager before dropping.");
        }
    }
}

