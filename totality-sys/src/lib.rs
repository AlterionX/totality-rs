extern crate totality_threading as th;
extern crate totality_model as geom;
extern crate nalgebra as na;
extern crate arrayvec as av;
extern crate shaderc;
extern crate log;

pub mod io;
pub mod renderer;

use totality_threading::killable_thread as kt;

#[macro_export]
macro_rules! cb_arc {
    ( $name:literal, $v:ident, $s:ident, $l_t:ident, $c_t:ident, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |$s: &$crate::io::e::State, $v: &$crate::io::e::V, $l_t: &std::time::Instant, $c_t: &std::time::Instant| {
                    trace!("{:?} update fired with {:?}", $name, $v);
                    $($head)*;
                    trace!("{:?} handler completed.", $name);
                }
            ));
            arc
        }
    };
    ( $name:literal, $v:ident, $s:ident, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |$s: &$crate::io::e::State, $v: &$crate::io::e::V, _: &std::time::Instant, _: &std::time::Instant| {
                    trace!("{:?} update fired with {:?}", $name, $v);
                    $($head)*;
                    trace!("{:?} handler completed.", $name);
                }
            ));
            arc
        }
    };
    ( $name:literal, $s:ident, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |$s: &$crate::io::e::State, v: &$crate::io::e::V, l_t: &std::time::Instant, c_t: &std::time::Instant| {
                    trace!("{:?} update fired with {:?}", $name, v);
                    $($head)*;
                    trace!("{:?} handler completed.", $name);
                }
            ));
            arc
        }
    };
    ( $name:literal, {$($head:tt)*} ) => {
        {
            use log::trace;
            let arc = std::sync::Arc::new(std::sync::Mutex::new(
                move |_: &$crate::io::e::State, v: &$crate::io::e::V, _: &std::time::Instant, _: &std::time::Instant| {
                    trace!("{:?} update fired with {:?}", $name, v);
                    $($head)*;
                    trace!("{:?} handler completed.", $name);
                }
            ));
            arc
        }
    };
}

