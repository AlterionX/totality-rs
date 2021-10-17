pub mod linkage;
use linkage::*;

use std::time::Duration;

use log::{info, trace};

pub trait PhysicsHook<T>: FnMut(&geom::scene::Static, &mut T) + Send + 'static {}
impl<T, F: FnMut(&geom::scene::Static, &mut T) + Send + 'static> PhysicsHook<T> for F {}

struct Simulation<T: Simulated, DL: DataLinkage<T>> {
    post_cbs: Vec<Box<dyn PhysicsHook<T>>>,
    pre_cbs: Vec<Box<dyn PhysicsHook<T>>>,
    time_step: Duration,
    dlink: DL,
}
unsafe impl<T: Simulated, DL: DataLinkage<T>> Send for Simulation<T, DL> {}
impl<T: Simulated, DL: DataLinkage<T>> Simulation<T, DL> {
    pub fn step(&mut self) {
        trace!("Simulating a single step.");
        // call pre
        // simulate
        if let Some(data) = self.dlink.advance() {
            // actually does exist, so lock and update
            T::step(self.time_step, data.source(), data.target());
        } else {
            panic!("Scene corrupted!")
        };
        // call post
    }
    pub fn as_thread(
        d: Duration,
        dlink: DL,
        post_cbs: Vec<Box<dyn PhysicsHook<T>>>,
        pre_cbs: Vec<Box<dyn PhysicsHook<T>>>,
    ) -> Result<th::killable_thread::KillableThread<(), ()>, std::io::Error> {
        th::create_duration_kt!(d, "Simulation", {
            let mut sim = {
                Simulation {
                    dlink,
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

pub struct Manager {
    sim_th: Option<th::killable_thread::KillableThread<(), ()>>,
}
impl Manager {
    pub fn new<T: Simulated, DL: DataLinkage<T>>(
        d: Duration,
        dl: DL,
        post_cbs: Vec<Box<dyn PhysicsHook<T>>>,
        pre_cbs: Vec<Box<dyn PhysicsHook<T>>>,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            sim_th: Some(Simulation::as_thread(d, dl, post_cbs, pre_cbs)?),
        })
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        info!("Shutting down simulation systems.");
        drop(self.sim_th.take());
    }
}
