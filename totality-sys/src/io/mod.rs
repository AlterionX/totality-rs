pub mod event;
pub mod cb;
mod source;

pub use self::event as e;
pub use self::source::WindowSpecs;
use self::source::IO;
use super::kt::KillableThread;

use std::{
    option::Option,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{channel, TryRecvError, Sender, Receiver, RecvError, SendError},
    },
    result::Result,
    time::{Instant, Duration},
};
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
        let (tx, rx) = channel();
        KillableThread::new(tx, "Immediate Event Loop".to_string(), move || {
            println!("Starting system state immediate thread.");
            let mut man = cb::Manager::new();
            let mut io = self::source::back::IO::new();
            io.init();
            if let Err(_) = win_tx.send(io.create_window(WindowSpecs::new("Tracer"))) {
                panic!("Could not send created window back to main thread.");
            };
            let target = Duration::from_secs(1).checked_div(600).expect("A constant is taken to be equal to 0...");
            loop {
                let curr_start_time = Instant::now();
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
                        let mut vv = Vec::with_capacity(10);
                        io.next_events(&mut vv);
                        // if vv.len() != 0 { man.fire_and_clean_listing(&mut *s_mg, &mut vv); }
                        man.fire_and_clean_listing(&mut *s_mg, &mut vv);
                    },
                    Err(_) => error!("State is poisoned."),
                };
                trace!("Checking for death.");
                match rx.try_recv() {
                    // Cannot handle messages
                    Ok(_) => panic!("Unexpected input into thread control channel."),
                    // No input means continue
                    Err(TryRecvError::Empty) => (),
                    // Outside was dropped, so stop this thread
                    Err(TryRecvError::Disconnected) => {
                        info!("Completed");
                        break
                    },
                };
                let busy_time = Instant::now() - curr_start_time;
                std::thread::sleep(target - busy_time);
                let total_time = Instant::now() - curr_start_time;
                trace!("{:?} spent busy in {:?} long loop.", busy_time, total_time);
            }
            info!("System state immediate thread winding down.");
        }).expect("Could not start event thread.... Welp I'm out.")
    }
    #[inline(always)]
    fn start_periodic_thread(
        s_m: Arc<Mutex<State>>,
        req_rx: Receiver<cb::RegRequest>,
        res_tx: Sender<cb::RegResponse>,
    ) -> KillableThread<()> {
        let (tx, rx) = channel();
        KillableThread::new(tx, "Periodic Event Loop".to_string(), move || {
            info!("Starting system state periodic thread.");
            let mut man = cb::Manager::new();
            let target = Duration::from_secs(1).checked_div(200).expect("A constant is taken to be equal to 0...");
            loop {
                let curr_start_time = Instant::now();
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
                trace!("Checking for death.");
                match rx.try_recv() {
                    // Cannot handle messages
                    Ok(c) => panic!("Unexpected input {:?} into thread control channel.", c),
                    // No input means continue
                    Err(TryRecvError::Empty) => (),
                    // Outside was dropped, so stop this thread
                    Err(TryRecvError::Disconnected) => {
                        info!("Completed");
                        break
                    },
                };
                let busy_time = Instant::now() - curr_start_time;
                std::thread::sleep(target - busy_time);
                let total_time = Instant::now() - curr_start_time;
                trace!("{:?} spent busy in {:?} long loop.", busy_time, total_time);
            }
            info!("System state periodic thread winding down.");
        }).expect("Could not start event thread.... Welp I'm out.")
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
pub type FinishResult = Option<(super::kt::FinishResult<()>, super::kt::FinishResult<()>)>;
impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(_) = self.pollers {
            panic!("Finish must be called on Manager before it can be dropped.")
        }
    }
}
