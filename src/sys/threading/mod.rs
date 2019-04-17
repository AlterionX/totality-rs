use std::{
    thread::JoinHandle,
    option::Option,
    sync::{
        mpsc::Sender,
    },
    result::Result,
};

pub struct KillableThread {
    kill_mechanism: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
}
impl KillableThread {
    pub fn new(s: Sender<()>, h: JoinHandle<()>) -> KillableThread {
        KillableThread { kill_mechanism: Option::Some(s), handle: Option::Some(h) }
    }
    pub fn finish(mut self) -> std::thread::Result<()> {
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
            "You MUST call cleanup on `sys::threading::KillableThread` to clean it up."
        );
    }
}
