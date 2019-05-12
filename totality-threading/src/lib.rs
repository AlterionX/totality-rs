//! # TotalityThreading
//!
//! Easily create heavy threads operating under various frequencies and conditions.
//!
//! Provides a variety of macros that help out with the thread creation. Also provides
//! trace-level logging statements.

extern crate proc_macro;
pub mod killable_thread;

/// A macro to create a simple KillableThread.
///
/// # Arguments
///
/// * `type` The fork's return type.
/// * `name` A string literal of the name of the thread. Used for logging.
/// * `head` The first part of the thread's execution. Could be used for setup. Variables declared
/// here are accessible to `body` and `head`.
/// * `body` The looped part of the body's execution.
/// * `tail` The last part of the thread's execution. Could be used for cleanup. Should return
/// value.
///
/// # Remarks
///
/// Don't sleep for a long time in the `body`!! If you do, the generated code cannot check for the
/// interrupted-ness of the thread.
#[macro_export]
macro_rules! create_kt {
    ( $type:ty, $name:literal, {$($head:tt)*}, {$($body:tt)*}, {$($tail:tt)*} ) => {
        {
            use log::*;
            use ::std::{
                time::*,
                sync::mpsc::*,
            };
            let (tx, rx) = std::sync::mpsc::channel();
            $crate::killable_thread::KillableThread::new(tx, $name.to_string(), move || -> $type {
                info!("Starting {} thread.", $name);
                $($head)*;
                loop {
                    let curr_start_time = Instant::now();
                    $($body)*;
                    trace!("Checking for {} thread's death.", $name);
                    match rx.try_recv() {
                        // Cannot handle messages
                        Ok(c) => panic!("Unexpected input {:?} into thread control channel.", c),
                        // No input means continue
                        Err(TryRecvError::Empty) => (),
                        // Outside was dropped, so stop this thread
                        Err(TryRecvError::Disconnected) => {
                            info!("{} thread completed", $name);
                            break
                        },
                    };
                    let busy_time = Instant::now() - curr_start_time;
                    trace!("{} thread spent {:?} busy in loop.", $name, busy_time);
                }
                let ret = {
                    $($tail)*
                };
                trace!("{} thread winding down.", $name);
                ret
            })
        }
    }
}

/// A macro to create a thread that aims to finish the loop with exactly a certain duration.
///
/// Refer to `create_kt` for more details.
///
/// # Arguments
///
/// * `type` The fork's return type.
/// * `dura` Get the target Duration per loop.
/// * `name` A string literal of the name of the thread. Used for logging.
/// * `head` The first part of the thread's execution. Could be used for setup. Variables declared
/// here are accessible to `body` and `head`.
/// * `body` The looped part of the body's execution.
/// * `tail` The last part of the thread's execution. Could be used for cleanup. Should return
/// value.
#[macro_export]
macro_rules! create_duration_kt {
    ( $type:ty, $dura:expr, $name:literal, {$($head:tt)*}, {$($body:tt)*}, {$($tail:tt)*} ) => {
        {
            use log::*;
            use ::std::{
                time::*,
                sync::mpsc::*,
            };
            use $crate::killable_thread::KillableThread;
            let (tx, rx) = std::sync::mpsc::channel();
            $crate::killable_thread::KillableThread::new(tx, $name.to_string(), move || -> $type {
                info!("Starting {} thread.", $name);
                $($head)*;
                let target = $dura;
                loop {
                    let curr_start_time = Instant::now();
                    $($body)*;
                    trace!("Checking for {} thread's death.", $name);
                    match rx.try_recv() {
                        // Cannot handle messages
                        Ok(c) => panic!("Unexpected input {:?} into thread control channel.", c),
                        // No input means continue
                        Err(TryRecvError::Empty) => (),
                        // Outside was dropped, so stop this thread
                        Err(TryRecvError::Disconnected) => {
                            info!("{} thread completed.", $name);
                            break
                        },
                    };
                    let busy_time = Instant::now() - curr_start_time;
                    if target > busy_time {
                        std::thread::sleep(target - busy_time);
                    }
                    let total_time = Instant::now() - curr_start_time;
                    trace!("{} thread spent {:?} busy and {:?} total in loop.", $name, busy_time, total_time);
                }
                let ret = {
                    $($tail)*
                };
                trace!("{} thread winding down.", $name);
                ret
            })
        }
    }
}

/// A macro to create a thread that aims to run the loop at a target rate (aka FPS).
///
/// Refer to `create_kt` for more details.
///
/// # Arguments
///
/// * `type` The fork's return type.
/// * `rate` An value denoting target fps. Must be nonzero.
/// * `name` A string literal of the name of the thread. Used for logging.
/// * `head` The first part of the thread's execution. Could be used for setup. Variables declared
/// here are accessible to `body` and `head`.
/// * `body` The looped part of the body's execution.
/// * `tail` The last part of the thread's execution. Could be used for cleanup. Should return
/// value.
#[macro_export]
macro_rules! create_rated_kt {
    ( $type:ty, $rate:expr, $name:literal, {$($head:tt)*}, {$($body:tt)*}, {$($tail:tt)*} ) => {
        {
            use log::*;
            use ::std::{
                time::*,
                sync::mpsc::*,
            };
            use $crate::killable_thread::KillableThread;
            let (tx, rx) = std::sync::mpsc::channel();
            $crate::killable_thread::KillableThread::new(tx, $name.to_string(), move || -> $type {
                info!("Starting {} thread.", $name);
                $($head)*;
                let target = Duration::from_secs(1).checked_div($rate).expect("A constant is taken to be equal to 0...");
                loop {
                    let curr_start_time = Instant::now();
                    $($body)*;
                    trace!("Checking for {} thread's death.", $name);
                    match rx.try_recv() {
                        // Cannot handle messages
                        Ok(c) => panic!("Unexpected input {:?} into thread control channel.", c),
                        // No input means continue
                        Err(TryRecvError::Empty) => (),
                        // Outside was dropped, so stop this thread
                        Err(TryRecvError::Disconnected) => {
                            info!("{} thread completed.", $name);
                            break
                        },
                    };
                    let busy_time = Instant::now() - curr_start_time;
                    if target > busy_time {
                        std::thread::sleep(target - busy_time);
                    }
                    let total_time = Instant::now() - curr_start_time;
                    trace!("{} thread spent {:?} busy and {:?} total in loop.", $name, busy_time, total_time);
                }
                let ret = {
                    $($tail)*
                };
                trace!("{} thread winding down.", $name);
                ret
            })
        }
    }
}

#[test]
pub fn run_test() {
    use log::info;

    let some_fun = vec![0, 1, 2, 3, 4];
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    match create_kt!(String, ":", {
        let mut hello = Vec::new();
        hello.extend(some_fun);
        let mut count = 0;
    }, {
        info!("We're looping!! We've reached this point {:?} times.", count);
        match tx.send(count.clone()) {
            Ok(()) => info!("Successfully sent integer!"),
            Err(_) => info!("Unexpected behavior on iteration {:?}!", count),
        }
        count += 1;
        if count == 10 {
            drop(tx);
            break
        }
    }, {
        info!("Welp, guess we're through here. Here's the vec, but the way: {:?}", hello);
        info!("We're early terminating to check if break works as expected.");
        "A random string is your reward!".to_string()
    }) {
        Ok(kt) => {
            for i in 0..10 {
                match rx.recv() {
                    Ok(r) => assert_eq!(r, i, "Oh crap, we received things in a weird way!"),
                    Err(e) => panic!("Not like this!!!! We've failed due to {:?}!", e),
                }
            }
            match rx.try_recv() {
                Ok(v) => panic!("We received a value ({:?}) after it should've been dropped...", v),
                Err(std::sync::mpsc::TryRecvError::Empty) => panic!("The channel should've dropped by now!"),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => info!("Successfully disconnected from the thread."),
            }
            match kt.finish() {
                Some(res) =>{
                        match res {
                        Ok(s) => assert_eq!("A random string is your reward!", s, "We should have gotten this string back..."),
                        Err(e) => panic!("Oh shit! We failed somewhere to join! Here's the error: {:?}", e),
                    }
                },
                None => panic!("We should have gotten something back!"),
            }
        },
        Err(_) => panic!("Unexpected error!"),
    }
    info!("No error!");
}
