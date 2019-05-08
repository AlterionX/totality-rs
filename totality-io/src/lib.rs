extern crate totality_threading as th;
extern crate nalgebra as na;
extern crate winit;
extern crate log;

// exports
pub mod event;
pub mod cb;
mod source;
pub use self::event as e;

// std dependencies
use std::{
    option::Option,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{channel, Sender, Receiver, RecvError, SendError},
    },
    result::Result,
};
// internal dependencies
use self::source::WindowSpecs;
use self::source::IO;
// workspace internal dependencies
use th::killable_thread::{self as kt, KillableThread};
// external dependencies
use winit::Window;
use self::e::*;

#[allow(dead_code)]
use log::{debug, warn, error, info, trace};

struct Twinned<T> {
    imm: T,
    per: T,
}
impl <T> Twinned<T> {
    fn consume<F>(self, f: F) where F: Fn(T) {
        f(self.imm);
        f(self.per);
    }
    fn map<F, U>(self, f: F) -> Twinned<U> where F: Fn(T) -> U {
        Twinned {
            imm: f(self.imm),
            per: f(self.per),
        }
    }
    fn as_tup(self) -> (T, T) {
        (self.imm, self.per)
    }
}

pub enum RegErr<T> {
    Send(SendError<T>),
    Recv(RecvError),
}

pub struct Manager {
    registrar: Twinned<Mutex<(Sender<cb::RegRequest>, Receiver<cb::RegResponse>)>>, // triggered periodically
    pollers: Option<Twinned<KillableThread<()>>>, // thread handle of polling thread
    pub win: Arc<self::source::back::Window>, // output is just this part
}
impl Manager {
    #[inline(always)]
    fn start_event_thread(
        s_m: Arc<Mutex<e::State>>,
        // window creation needs to happen here
        win_tx: Sender<Window>,
        req_rx: Receiver<cb::RegRequest>,
        res_tx: Sender<cb::RegResponse>,
    ) -> KillableThread<()> {
        th::create_kt!((), "Immediate Event Loop", {
            let mut man = cb::Manager::new();
            let mut io = self::source::back::IO::new();
            io.init();
            if let Err(_) = win_tx.send(io.create_window(WindowSpecs::new("Tracer"))) {
                panic!("Could not send created window back to main thread.");
            };
            let mut vv = Vec::with_capacity(10);
        }, {
            trace!("Pulling callbacks.");
            loop {
                match req_rx.try_recv() {
                    // Cannot handle messages
                    Ok(req) => match res_tx.send(man.handle_req(req)) {
                        Ok(_) => trace!("Request completed fully."),
                        Err(_) => warn!("Request completed, but response could not be sent."),
                    },
                    // If any error occurs, break. Dropping is handled elsewhere
                    Err(TryRecvError::Empty) => break,
                    // Outside was dropped, so stop this thread
                    Err(TryRecvError::Disconnected) => {
                        warn!("Request channel dropped prior to exit.");
                        break
                    },
                }
            }
            trace!("Firing all callbacks.");
            match s_m.lock() {
                Ok(mut s_mg) => {
                    io.next_events(&mut vv);
                    // if vv.len() != 0 { man.fire_and_clean_listing(&mut *s_mg, &mut vv); }
                    // TODO change to update state per event
                    man.fire_and_clean_listing(&mut *s_mg, &mut vv);
                    for v in vv.drain(..) {
                        (*s_mg).update(&v);
                    }
                },
                Err(_) => error!("State is poisoned."),
            };
        }, {}).expect("Could not start event thread.... Welp I'm out.")
    }
    #[inline(always)]
    fn start_periodic_thread(
        s_m: Arc<Mutex<State>>,
        req_rx: Receiver<cb::RegRequest>,
        res_tx: Sender<cb::RegResponse>,
    ) -> KillableThread<()> {
        th::create_kt!((), "Periodic Event Loop", {
            let mut man = cb::Manager::new();
        }, {
            trace!("Pulling callbacks.");
            loop {
                match req_rx.try_recv() {
                    // Cannot handle messages
                    Ok(req) => match res_tx.send(man.handle_req(req)) {
                        Ok(_) => trace!("Request completed fully."),
                        Err(_) => warn!("Request completed, but response could not be sent."),
                    },
                    // If any error occurs, break. Dropping is handled elsewhere
                    Err(TryRecvError::Empty) => break,
                    // Outside was dropped, so stop this thread
                    Err(TryRecvError::Disconnected) => {
                        warn!("Request channel dropped prior to exit.");
                        break
                    },
                }
            }
            trace!("Firing all callbacks.");
            match s_m.lock() {
                Ok(s_mg) => man.fire_and_clean_all(&*s_mg),
                Err(_) => error!("State is poisoned."),
            };
        }, {}).expect("Could not start event thread.... Welp I'm out.")
    }
    pub fn new() -> Manager {
        let (win_tx, win_rx) = channel();
        let (imm_tx, imm_rx) = channel();
        let (w_imm_tx, w_imm_rx) = channel();
        let (per_tx, per_rx) = channel();
        let (w_per_tx, w_per_rx) = channel();
        let curr = Arc::new(Mutex::new(State::default()));
        Manager {
            registrar: Twinned {
                per: Mutex::new((per_tx, w_per_rx)),
                imm: Mutex::new((imm_tx, w_imm_rx)),
            },
            pollers: Option::Some(Twinned {
                imm: Self::start_event_thread(curr.clone(), win_tx, imm_rx, w_imm_tx),
                per: Self::start_periodic_thread(curr, per_rx, w_per_tx),
            }),
            win: Arc::new(win_rx.recv().unwrap()),
        }
    }
    pub fn reg_imm<F>(&self, c: C, f: Arc<Mutex<F>>) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> where F: cb::CBFn {
        let cb = Arc::downgrade(&f);
        Self::send_to_manager(&self.registrar.imm, cb::RegRequest::Register(c, vec![cb::CB::new(c, cb)]))
    }
    pub fn reg_per<F>(&self, c: C, f: Arc<Mutex<F>>) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> where F: cb::CBFn {
        let cb = Arc::downgrade(&f);
        Self::send_to_manager(&self.registrar.per, cb::RegRequest::Register(c, vec![cb::CB::new(c, cb)]))
    }
    pub fn unreg_imm(&self, cb: Weak<Mutex<cb::CB>>) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> {
        Self::send_to_manager(&self.registrar.imm, cb::RegRequest::Unregister(vec![cb]))
    }
    pub fn unreg_per(&self, cb: Weak<Mutex<cb::CB>>) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> {
        Self::send_to_manager(&self.registrar.per, cb::RegRequest::Unregister(vec![cb]))
    }
    fn send_to_manager(trx: &Mutex<(Sender<cb::RegRequest>, Receiver<cb::RegResponse>)>, req: cb::RegRequest) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> {
        match trx.lock() {
            Ok(guard) => {
                let (ref tx, ref rx) = *guard;
                match tx.send(req) {
                    Ok(_) => match rx.recv() {
                        Ok(res) => Result::Ok(res),
                        Err(e) => Result::Err(RegErr::Recv(e))
                    },
                    Err(e) => Result::Err(RegErr::Send(e)),
                }
            },
            Err(_) => panic!("Fack. Dropping all the registrations.")
        }
    }
    pub fn finish(mut self) -> FinishResult {
        self.pollers.take().map(|pollers| pollers.map(|p| p.finish()).as_tup())
    }
}
pub type FinishResult = Option<(kt::FinishResult<()>, kt::FinishResult<()>)>;
impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(_) = self.pollers {
            panic!("Finish must be called on Manager before it can be dropped.")
        }
    }
}

#[macro_export]
macro_rules! cb_arc {
    ( $name:literal, $v:ident, $s:ident, $l_t:ident, $c_t:ident, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |$s: &$crate::e::State, $v: &$crate::e::V, $l_t: &std::time::Instant, $c_t: &std::time::Instant| {
                    trace!("{} handler fired with {:?}", $name, $v);
                    $($head)*;
                    trace!("{} handler completed.", $name);
                }
            ));
            arc
        }
    };
    ( $name:literal, $v:ident, $s:ident, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |$s: &$crate::e::State, $v: &$crate::e::V, _: &std::time::Instant, _: &std::time::Instant| {
                    trace!("{} handler fired with {:?}", $name, $v);
                    $($head)*;
                    trace!("{} handler completed.", $name);
                }
            ));
            arc
        }
    };
    ( $name:literal, $s:ident, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |$s: &$crate::e::State, v: &$crate::e::V, l_t: &std::time::Instant, c_t: &std::time::Instant| {
                    trace!("{} handler fired with {:?}", $name, v);
                    $($head)*;
                    trace!("{} handler completed.", $name);
                }
            ));
            arc
        }
    };
    ( $name:literal, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |_: &$crate::e::State, v: &$crate::e::V, _: &std::time::Instant, _: &std::time::Instant| {
                    trace!("{} handler fired with {:?}", $name, v);
                    $($head)*;
                    trace!("{} handler completed.", $name);
                }
            ));
            arc
        }
    };
}

