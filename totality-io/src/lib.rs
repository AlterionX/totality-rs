// exports
mod source;

use internal_events::cb;
pub use internal_events::hal as e;

// std dependencies
use std::{
    option::Option,
    result::Result,
    sync::{
        mpsc::{channel, Receiver, RecvError, SendError, Sender},
        Arc, Mutex, Weak,
    },
};
// internal dependencies
use self::source::WindowSpecs;
use self::source::IO;
// workspace internal dependencies
use th::killable_thread::KillableThread;
// external dependencies
use e::*;
use winit::Window;

use log::{error, info, trace, warn};

struct Twinned<T> {
    imm: T,
    per: T,
}

pub enum RegErr<T> {
    Send(SendError<T>),
    Recv(RecvError),
}

pub type CB = cb::CB<State, V, C>;

pub struct Manager {
    registrar: Twinned<
        Mutex<(
            Sender<cb::RegRequest<State, V, C>>,
            Receiver<cb::RegResponse<State, V, C>>,
        )>,
    >, // triggered periodically
    pollers: Option<Twinned<KillableThread<(), ()>>>, // thread handle of polling thread
    pub win: Arc<self::source::back::Window>,         // output is just this part
}
impl Manager {
    #[inline(always)]
    fn start_event_thread(
        s_m: Arc<Mutex<e::State>>,
        // window creation needs to happen here
        win_tx: Sender<Window>,
        req_rx: Receiver<cb::RegRequest<State, V, C>>,
        res_tx: Sender<cb::RegResponse<State, V, C>>,
    ) -> KillableThread<(), ()> {
        th::create_kt!(
            "Immediate Event Loop",
            {
                let mut man = cb::Manager::new();
                let mut io = self::source::back::IO::new();
                io.init();
                if let Err(_) = win_tx.send(io.create_window(WindowSpecs::new("Tracer"))) {
                    panic!("Could not send created window back to main thread.");
                };
                let mut vv = Vec::with_capacity(10);
            },
            {
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
                            break;
                        }
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
                    }
                    Err(_) => error!("State is poisoned."),
                };
            },
            {}
        )
        .expect("Could not start event thread.... Welp I'm out.")
    }
    #[inline(always)]
    fn start_periodic_thread(
        s_m: Arc<Mutex<State>>,
        req_rx: Receiver<cb::RegRequest<State, V, C>>,
        res_tx: Sender<cb::RegResponse<State, V, C>>,
    ) -> KillableThread<(), ()> {
        th::create_rated_kt!(
            60,
            "Periodic Event Loop",
            {
                let mut man = cb::Manager::new();
            },
            {
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
                            break;
                        }
                    }
                }
                trace!("Firing all callbacks.");
                match s_m.lock() {
                    Ok(s_mg) => man.fire_and_clean_all(&*s_mg),
                    Err(_) => error!("State is poisoned."),
                };
            },
            {}
        )
        .expect("Could not start event thread.... Welp I'm out.")
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
    pub fn reg_imm<F: cb::CBFn<State, V, C>>(
        &self,
        c: C,
        f: Arc<Mutex<F>>,
    ) -> Result<cb::RegResponse<State, V, C>, RegErr<cb::RegRequest<State, V, C>>> {
        let cb = Arc::downgrade(&f);
        Self::send_to_manager(
            &self.registrar.imm,
            cb::RegRequest::Register(c, vec![cb::CB::new(c, cb)]),
        )
    }
    pub fn reg_per<F: cb::CBFn<State, V, C>>(
        &self,
        c: C,
        f: Arc<Mutex<F>>,
    ) -> Result<cb::RegResponse<State, V, C>, RegErr<cb::RegRequest<State, V, C>>> {
        let cb = Arc::downgrade(&f);
        Self::send_to_manager(
            &self.registrar.per,
            cb::RegRequest::Register(c, vec![cb::CB::new(c, cb)]),
        )
    }
    pub fn unreg_imm(
        &self,
        cb: Weak<Mutex<cb::CB<State, V, C>>>,
    ) -> Result<cb::RegResponse<State, V, C>, RegErr<cb::RegRequest<State, V, C>>> {
        Self::send_to_manager(&self.registrar.imm, cb::RegRequest::Unregister(vec![cb]))
    }
    pub fn unreg_per(
        &self,
        cb: Weak<Mutex<cb::CB<State, V, C>>>,
    ) -> Result<cb::RegResponse<State, V, C>, RegErr<cb::RegRequest<State, V, C>>> {
        Self::send_to_manager(&self.registrar.per, cb::RegRequest::Unregister(vec![cb]))
    }
    fn send_to_manager(
        trx: &Mutex<(
            Sender<cb::RegRequest<State, V, C>>,
            Receiver<cb::RegResponse<State, V, C>>,
        )>,
        req: cb::RegRequest<State, V, C>,
    ) -> Result<cb::RegResponse<State, V, C>, RegErr<cb::RegRequest<State, V, C>>> {
        match trx.lock() {
            Ok(guard) => {
                let (ref tx, ref rx) = *guard;
                match tx.send(req) {
                    Ok(_) => match rx.recv() {
                        Ok(res) => Result::Ok(res),
                        Err(e) => Result::Err(RegErr::Recv(e)),
                    },
                    Err(e) => Result::Err(RegErr::Send(e)),
                }
            }
            Err(_) => panic!("Fack. Dropping all the registrations."),
        }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        info!("Shutting down system management.");
        drop(self.pollers.take())
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
