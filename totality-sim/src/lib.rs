extern crate totality_threading as th;
extern crate totality_model as geom;
extern crate log;

use std::{
    sync::{Arc, Mutex, Weak},
    time::{Duration},
};

trait PhysicsHook: FnMut(&geom::scene::Static, &mut geom::scene::Dynamic) + Send + 'static {}
impl <F: FnMut(&geom::scene::Static, &mut geom::scene::Dynamic) + Send + 'static> PhysicsHook for F {}

struct Simulation {
    mutated: Arc<Mutex<geom::scene::Scene>>,
    post_cbs: Vec<Box<PhysicsHook>>,
    pre_cbs: Vec<Box<PhysicsHook>>,
    time_step: Duration,
}
unsafe impl Send for Simulation {}
impl Simulation {
    fn dur_as_f64(d: &Duration) -> f64 {
        (d.as_secs() as f64) + (d.subsec_nanos() as f64) / 1_000_000f64
    }
    pub fn step(&mut self) {
        // call pre
        // simulate
        // call post
    }
    pub fn as_thread(d: Duration, sc: Arc<Mutex<geom::scene::Scene>>, post_cbs: Vec<Box<PhysicsHook>>, pre_cbs: Vec<Box<PhysicsHook>>) -> Result<th::killable_thread::KillableThread<()>, std::io::Error> {
        th::create_duration_kt!((), d, "Simulation", {
            let mut sim = Simulation {
                mutated: sc,
                post_cbs: post_cbs,
                pre_cbs: pre_cbs,
                time_step: d,
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
