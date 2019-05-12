//! # KillableThread
//!
//! A KillableThread is a thread with a built-in interruption mechanism/flag.

use std::{
    thread::JoinHandle,
    option::Option,
    sync::mpsc::Sender,
    result::Result,
};

/// A `KillableThread`. Effectively a `JoinHandle` to the thread started by when creating
/// `KillableThread`.
pub struct KillableThread<T: Send + 'static> {
    kill_mechanism: Option<Sender<()>>,
    handle: Option<JoinHandle<T>>,
}
impl <T: Send + 'static> KillableThread<T> {
    /// Creates a `KillableThread`.
    ///
    /// # Arguments
    ///
    /// * `s` The mpsc Sender responsible for notifying the function `f` if it should halt.
    /// * `name` Name of the KillableThread. Used during debugging only.
    /// * `f` The function being run by the thread.
    ///
    /// # Remarks
    ///
    /// You must manually check if sender is dropped. If you wish for a less optimized, but easier
    /// way to do this, check out the macros provided at the crate's top level.
    pub fn new<F: FnOnce() -> T + Send + 'static>(s: Sender<()>, name: String, f: F) -> Result<KillableThread<T>, std::io::Error> {
        match std::thread::Builder::new().name(name).spawn(f) {
            Ok(h) => Result::Ok(KillableThread {
                kill_mechanism: Option::Some(s),
                handle: Option::Some(h)
            }),
            Err(e) => Result::Err(e)
        }
    }
    /// Effectively the same as `join` in a `JoinHandle`.
    ///
    /// # Remarks
    ///
    /// If called more than once, will return None on subsequent calls.
    pub fn finish(mut self) -> Option<std::thread::Result<T>> {
        drop(self.kill_mechanism.take());
        self.handle.take().map(|h| h.join())
    }
}
/// Alias for the return of `finish` in `KillableThread`.
pub type FinishResult<T> = Option<std::thread::Result<T>>;
impl <T: Send + 'static> Drop for KillableThread<T> {
    fn drop(&mut self) {
        assert!(
            self.kill_mechanism.is_none() && self.handle.is_none(),
            "You MUST call cleanup on `sys::threading::KillableThread` to clean it up."
        );
    }
}
