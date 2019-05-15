//! Link together otherwise independent systems together.

use std::{
    cell::UnsafeCell,
    sync::{Arc, Mutex},
};

use gui::{draw, linkage as gl};
use ren::rp as rl;
use sim::linkage as sl;
use sync::triple_buffer as tb;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

// gui
pub struct EventSystemLinkage {
    // TODO register for needed events, possibly prevent other events/link to those systems as well
}
impl gl::EventLinkage for EventSystemLinkage {}
pub struct RenderSystemLinkage {
    // TODO update render state / cache, possibly prevent other events/link to those systems as well
}
impl gl::DrawLinkage for RenderSystemLinkage {
    fn queue_cmd(&self, cmd: draw::Cmd) {}
    fn disptach(&self) {}
}

// ren
pub struct RenderData {
    opt_rv: Option<UnsafeCell<Option<tb::Reading<geom::scene::Dynamic>>>>,
    opt_st: Option<Arc<geom::scene::Static>>,
    should_use_depth: Arc<Mutex<bool>>,
    should_restart_renderer: Arc<Mutex<bool>>,
    fish: Arc<Mutex<i32>>,
    color: Arc<Mutex<na::Vector4<f32>>>,
    camera: Arc<Mutex<geom::camera::Camera>>,
}
impl RenderData {
    pub fn new(
        opt_rv: Option<tb::ReadingView<geom::scene::Dynamic>>,
        opt_st: Option<Arc<geom::scene::Static>>,
        should_use_depth: Arc<Mutex<bool>>,
        should_restart_renderer: Arc<Mutex<bool>>,
        fish: Arc<Mutex<i32>>,
        color: Arc<Mutex<na::Vector4<f32>>>,
        cam: Arc<Mutex<geom::camera::Camera>>,
    ) -> Self {
        Self {
            opt_rv: opt_rv.map(|rv| UnsafeCell::new(Some(tb::Reading::ReadingView(rv)))),
            opt_st: opt_st,
            should_use_depth,
            should_restart_renderer,
            fish,
            color,
            camera: cam,
        }
    }
    fn lock(&self) -> Option<&tb::Reader<geom::scene::Dynamic>> {
        if let Some(ref rv) = self.opt_rv {
            let rv_ptr = rv.get();
            match unsafe { (*rv_ptr).take() } {
                Some(tb::Reading::ReadingView(rv)) => {
                    unsafe { (*rv_ptr).replace(tb::Reading::Reader(rv.read())) };
                    if let Some(tb::Reading::Reader(ref r)) = unsafe { &*rv_ptr } {
                        Some(r)
                    } else {
                        None
                    }
                }
                Some(e) => {
                    unsafe { (*rv_ptr).replace(e) };
                    None
                }
                None => None,
            }
        } else {
            None
        }
    }
    fn unlock(&self) -> Option<&tb::ReadingView<geom::scene::Dynamic>> {
        if let Some(ref r) = self.opt_rv {
            let r_ptr = r.get();
            match unsafe { (*r_ptr).take() } {
                Some(tb::Reading::Reader(r)) => {
                    unsafe { (*r_ptr).replace(tb::Reading::ReadingView(r.release())) };
                    if let Some(tb::Reading::ReadingView(ref rv)) = unsafe { &*r_ptr } {
                        Some(rv)
                    } else {
                        None
                    }
                }
                Some(e) => {
                    unsafe { (*r_ptr).replace(e) };
                    None
                }
                None => None,
            }
        } else {
            None
        }
    }
}
impl rl::DataLinkage<ren::IT> for RenderData {
    fn next_req(&self) -> Option<ren::RenderReq<ren::IT>> {
        use ren::*;
        let dy = if let Some(dy) = self.lock() {
            dy
        } else {
            return None;
        };
        let should_depth = *self.should_use_depth.lock().expect("Seriously?");
        let mut restart = self.should_restart_renderer.lock().expect("Seriously?");
        let draw_id = *self.fish.lock().expect("Seriously?");
        let original_color = self.color.lock().expect("Seriously?");
        let cam_clone = self.camera.lock().expect("Camera poisoned!").clone();
        let st = if let Some(ref st) = self.opt_st {
            st
        } else {
            return None;
        };
        let req = if *restart {
            info!("Sending recreate instruction!");
            *restart = false;
            Some(RenderReq::Restart)
        } else if draw_id == 0 {
            let model_clone = dy.r().mm[draw_id as usize].clone();
            Some(RenderReq::DrawGroupWithSetting(
                vec![model_clone],
                cam_clone,
                Color(na::Vector4::new(
                    original_color[0] as f32,
                    original_color[1] as f32,
                    original_color[2] as f32,
                    original_color[3] as f32,
                )),
                RenderSettings {
                    should_use_depth: should_depth,
                },
            ))
        } else {
            let model_clones = vec![dy.r().mm[1].clone(), dy.r().mm[2].clone()];
            if let Ok(sud_g) = self.should_use_depth.lock() {
                Some(RenderReq::DrawGroupWithSetting(
                    model_clones,
                    cam_clone,
                    Color(na::Vector4::new(
                        original_color[0] as f32,
                        original_color[1] as f32,
                        original_color[2] as f32,
                        original_color[3] as f32,
                    )),
                    RenderSettings {
                        should_use_depth: *sud_g,
                    },
                ))
            } else {
                Some(RenderReq::DrawGroup(
                    model_clones,
                    cam_clone,
                    ren::Color(na::Vector4::new(
                        original_color[0] as f32,
                        original_color[1] as f32,
                        original_color[2] as f32,
                        original_color[3] as f32,
                    )),
                ))
            }
        };
        self.unlock();
        req
    }
}
unsafe impl Send for RenderData {}

// sim
pub struct SimulatedScene(Arc<geom::scene::Static>, *const geom::scene::Dynamic);
impl SimulatedScene {
    fn dur_as_f64(d: &std::time::Duration) -> f64 {
        (d.as_secs() as f64) + (d.subsec_nanos() as f64) / 1_000_000_000f64
    }
}
impl sl::Simulated for SimulatedScene {
    fn step(step: std::time::Duration, source: &Self, target: &mut Self) {
        let step = Self::dur_as_f64(&step) as f32;
        for (r_ele, w_ele) in unsafe {
            (*source.1)
                .mm
                .iter()
                .zip((*(target.1 as *mut geom::scene::Dynamic)).mm.iter_mut())
        } {
            w_ele.set_state(
                r_ele.pos + r_ele.vel * step,
                r_ele.vel,
                r_ele.ori * na::UnitQuaternion::identity().nlerp(&r_ele.omg, step),
                r_ele.omg,
                r_ele.scale,
            );
        }
    }
}
pub struct SimData {
    arc_st: Arc<geom::scene::Static>,
    e_state: UnsafeCell<Option<tb::Editing<geom::scene::Dynamic>>>,
    _sc_c: UnsafeCell<Option<SimulatedScene>>,
    _sc_m: UnsafeCell<Option<SimulatedScene>>,
}
impl SimData {
    pub fn new(ast: Arc<geom::scene::Static>, est: tb::Editing<geom::scene::Dynamic>) -> Self {
        Self {
            arc_st: ast,
            e_state: UnsafeCell::new(Some(est)),
            _sc_c: UnsafeCell::new(None),
            _sc_m: UnsafeCell::new(None),
        }
    }
}
impl sl::DataLinkage<SimulatedScene> for SimData {
    fn advance(&self) -> Option<sl::DataLinkageGuard<SimulatedScene, SimData>> {
        let s = self.e_state.get();
        match unsafe { (*s).take() } {
            Some(tb::Editing::EditingView(ev)) => {
                unsafe { (*s).replace(tb::Editing::Editor(ev.edit())) };
                // update _sc_m, _sc_p
                if let Some(tb::Editing::Editor(ref dy)) = unsafe { &*self.e_state.get() } {
                    unsafe {
                        (*self._sc_c.get()).replace(SimulatedScene(
                            self.arc_st.clone(),
                            dy.r() as *const geom::scene::Dynamic,
                        ));
                        (*self._sc_m.get()).replace(SimulatedScene(
                            self.arc_st.clone(),
                            dy.w() as *const geom::scene::Dynamic,
                        ));
                    };
                    Some(sl::DataLinkageGuard::new(self))
                } else {
                    None
                }
            }
            Some(e) => {
                error!("Attempting to lock Editor without unlocking!");
                unsafe { (*s).replace(e) };
                None
            }
            None => None,
        }
    }
    fn source(&self) -> Option<&SimulatedScene> {
        match unsafe { &*self._sc_c.get() } {
            Some(ref sc) => Some(sc),
            None => None,
        }
    }
    fn target(&self) -> Option<&mut SimulatedScene> {
        match unsafe { &mut *self._sc_m.get() } {
            Some(ref mut sc) => Some(sc),
            None => None,
        }
    }
    fn cleanup(&self) {
        let s = self.e_state.get();
        unsafe {
            match (*s).take() {
                Some(tb::Editing::Editor(e)) => {
                    (*s).replace(tb::Editing::EditingView(e.release()));
                }
                Some(e) => {
                    error!("Attempting to unlock Editor without locking!");
                    std::mem::forget((*s).replace(e));
                }
                None => (),
            }
        }
    }
}
unsafe impl Send for SimData {}
