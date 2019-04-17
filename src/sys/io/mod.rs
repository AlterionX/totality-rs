pub mod event;
pub mod cb;
mod source;

pub use self::event as e;
pub use self::source::WindowSpecs;
use self::source::IO;

use std::{
    thread::JoinHandle,
    option::Option,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{channel, TryRecvError, Sender, Receiver, RecvError, SendError},
    },
    result::Result,
    time::{Duration,Instant},
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
}
struct KillableThread {
    kill_mechanism: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
}
impl KillableThread {
    fn new(s: Sender<()>, h: JoinHandle<()>) -> KillableThread {
        KillableThread { kill_mechanism: Option::Some(s), handle: Option::Some(h) }
    }
    fn finish(mut self) -> std::thread::Result<()> {
        drop(self.kill_mechanism.take());
        if let Some(h) = self.handle.take() {
            h.join()
        } else { Result::Ok(()) }
    }
}
impl Drop for KillableThread {
    fn drop(&mut self) {
        assert!(
            self.kill_mechanism.is_none() && self.handle.is_none(),
            "You MUST call cleanup on `sys::Manager` to clean it up."
        );
    }
}

pub enum RegErr<T> {
    Send(SendError<T>),
    Recv(RecvError),
}

pub struct Manager {
    registrar: Twinned<Mutex<(Sender<cb::RegRequest>, Receiver<cb::RegResponse>)>>, // triggered periodically
    pollers: Option<Twinned<KillableThread>>, // thread handle of polling thread
    pub win: self::source::back::Window, // output is just this part
}
impl Manager {
    #[inline(always)]
    fn start_event_thread(
        s_m: Arc<Mutex<e::State>>,
        // window creation needs to happen here
        win_tx: Sender<Window>,
        req_rx: Receiver<cb::RegRequest>,
        res_tx: Sender<cb::RegResponse>,
    ) -> KillableThread {
        let (tx, rx) = channel();
        KillableThread::new(tx, std::thread::spawn(move || {
            println!("Starting system state immediate thread.");
            let mut man = cb::Manager::new();
            let mut io = self::source::back::IO::new();
            io.init();
            if let Err(_) = win_tx.send(io.create_window(WindowSpecs::new("Tracer"))) {
                panic!("Could not send created window back to main thread.");
            };
            info!("System state immediate thread entering loop.");
            let mut last_time = Instant::now();
            loop {
                let curr_time = Instant::now();
                info!("Time since last immediate iteration: {:?}", curr_time - last_time);
                match s_m.lock() {
                    Ok(mut s_mg) => {
                        let mut vv = Vec::with_capacity(10);
                        io.next_events(&mut vv);
                        info!("Handled {:?} immediate events.", vv.len());
                        // if vv.len() != 0 { man.fire_and_clean_listing(&mut *s_mg, &mut vv); }
                        man.fire_and_clean_listing(&mut *s_mg, &mut vv);
                    },
                    Err(_) => (),
                };
                loop {
                    match req_rx.try_recv() {
                        // Cannot handle messages
                        Ok(req) => match res_tx.send(man.handle_req(req)) {
                            Ok(_) => (), // everything is fine
                            Err(_) => (), // Don't react, the last part will take care of it. ... Well, should.
                        },
                        // If any error occurs, break. Dropping is handled elsewhere
                        Err(_) => break,
                    }
                }
                last_time = curr_time;
                match rx.try_recv() {
                    // Cannot handle messages
                    Ok(_) => panic!("Unexpected input"),
                    // No input means continue
                    Err(TryRecvError::Empty) => continue,
                    // Outside was dropped, so stop this thread
                    Err(TryRecvError::Disconnected) => break,
                };
            }
            info!("System state immediate thread winding down.");
        }))
    }
    #[inline(always)]
    fn start_periodic_thread(
        s_m: Arc<Mutex<State>>,
        req_rx: Receiver<cb::RegRequest>,
        res_tx: Sender<cb::RegResponse>,
    ) -> KillableThread {
        let (tx, rx) = channel();
        KillableThread::new(tx, std::thread::spawn(move || {
            info!("Starting system state periodic thread.");
            let mut man = cb::Manager::new();
            let mut last_time = Instant::now();
            loop {
                let curr_time = Instant::now();
                info!("Time since last periodic iteration: {:?}", curr_time - last_time);
                match s_m.lock() {
                    Ok(s_mg) => man.fire_and_clean_all(&*s_mg),
                    Err(_) => (),
                };
                loop {
                    match req_rx.try_recv() {
                        // Cannot handle messages
                        Ok(req) => match res_tx.send(man.handle_req(req)) {
                            Ok(_) => (), // everything is fine
                            Err(_) => (), // Don't react, the last part will take care of it.
                        },
                        // If any error occurs, break. Dropping is handled elsewhere
                        Err(_) => break,
                    }
                }
                last_time = curr_time;
                match rx.try_recv() {
                    // Cannot handle messages
                    Ok(_) => panic!("Unexpected input"),
                    // No input means continue
                    Err(TryRecvError::Empty) => continue,
                    // Outside was dropped, so stop this thread
                    Err(TryRecvError::Disconnected) => break,
                };
            }
            info!("System state periodic thread winding down.");
        }))
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
                per: Self::start_periodic_thread(curr.clone(), per_rx, w_per_tx),
            }),
            win: win_rx.recv().unwrap(),
        }
    }
    pub fn reg_imm<F>(&self, c: C, f: Arc<Mutex<F>>) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> where F: cb::CBFn {
        let cb = Arc::downgrade(&f);
        Self::send_to_manager(&self.registrar.imm, cb::RegRequest::Register(c, vec![cb::CB::new(c.clone(), cb)]))
    }
    pub fn reg_per<F>(&self, c: C, f: Arc<Mutex<F>>) -> Result<cb::RegResponse, RegErr<cb::RegRequest>> where F: cb::CBFn {
        let cb = Arc::downgrade(&f);
        Self::send_to_manager(&self.registrar.per, cb::RegRequest::Register(c, vec![cb::CB::new(c.clone(), cb)]))
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
    pub fn finish(mut self) -> Result<(), (std::thread::Result<()>, std::thread::Result<()>)> {
        // Drops input to trigger thread stops for poller
        if let Some(pollers) = self.pollers.take() {
            pollers.consume(|p| p.finish().expect("Meh, I was going to finish anyways."));
        }
        std::result::Result::Ok(())
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(_) = self.pollers {
            panic!("Finish must be called on Manager before it can be dropped.")
        }
    }
}
