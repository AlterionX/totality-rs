use std::{
    thread::JoinHandle,
    option::Option,
    sync::mpsc::Sender,
    result::Result,
};

pub struct KillableThread<T: Send + 'static> {
    kill_mechanism: Option<Sender<()>>,
    handle: Option<JoinHandle<T>>,
}
impl <T: Send + 'static> KillableThread<T> {
    pub fn new<F: FnOnce() -> T + Send + 'static>(s: Sender<()>, name: String, f: F) -> Result<KillableThread<T>, std::io::Error> {
        match std::thread::Builder::new().name(name).spawn(f) {
            Ok(h) => Result::Ok(KillableThread {
                kill_mechanism: Option::Some(s),
                handle: Option::Some(h)
            }),
            Err(e) => Result::Err(e)
        }
    }
    pub fn finish(mut self) -> Option<std::thread::Result<T>> {
        drop(self.kill_mechanism.take());
        self.handle.take().map(|h| h.join())
    }
}
pub type FinishResult<T> = Option<std::thread::Result<T>>;
impl <T: Send + 'static> Drop for KillableThread<T> {
    fn drop(&mut self) {
        assert!(
            self.kill_mechanism.is_none() && self.handle.is_none(),
            "You MUST call cleanup on `sys::threading::KillableThread` to clean it up."
        );
    }
}
