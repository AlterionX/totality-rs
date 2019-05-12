pub mod change;
pub mod axis;
pub mod pos;
pub mod button;

pub use self::axis as a;
pub use self::pos as p;
pub use self::button as b;

use std::collections::HashMap;
use crate::cb::{Categorized, ValueStore};

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum C {
    A(a::C),
    P(p::C),
    B(b::C),
    Ignored,
    Unknown,
}
impl C {
    pub fn default_v(&self) -> V {
        match self {
            C::A(a_c) => V::A(a::V::default_value_of(a_c)),
            C::P(p_c) => V::P(p::V::default_value_of(p_c)),
            C::B(b_c) => V::B(b::V::default_value_of(b_c)),
            Ignored => V::Ignored,
            _ => unimplemented!("Category {:?} does not have a default value yet.", self),
        }
    }
}
impl From<V> for C {
    fn from(v: V) -> C {
        match v {
            V::A(v) => C::from(a::C::from(v)),
            V::P(v) => C::from(p::C::from(v)),
            V::B(v) => C::from(b::C::from(v)),
            _ => unimplemented!("Crap. Can't convert from {:?} to C yet.", v),
        }
    }
}
impl From<&V> for C {
    fn from(v: &V) -> C {
        match v {
            V::A(v) => C::from(a::C::from(v)),
            V::P(v) => C::from(p::C::from(v)),
            V::B(v) => C::from(b::C::from(v)),
            _ => unimplemented!("Crap. Can't convert from {:?} to C yet.", v),
        }
    }
}
impl From<a::C> for C {
    fn from(c: a::C) -> C { C::A(c) }
}
impl From<p::C> for C {
    fn from(k: p::C) -> C { C::P(k) }
}
impl From<b::C> for C {
    fn from(c: b::C) -> C { C::B(c) }
}

#[derive(Debug, Copy, Clone)]
pub enum V {
    A(a::V),
    P(p::V),
    B(b::V),
    Ignored,
    Unknown,
}
impl Categorized<C> for V {
    fn category(&self) -> C {
        C::from(self)
    }
}

#[derive(Default)]
pub struct State {
    m: HashMap<C, V>
}
impl From<a::V> for V {
    fn from(v: a::V) -> V { V::A(v) }
}
impl From<p::V> for V {
    fn from(v: p::V) -> V { V::P(v) }
}
impl From<b::V> for V {
    fn from(v: b::V) -> V { V::B(v) }
}
impl State {
    pub fn update<'a>(&mut self, v: &'a V) -> (C, &'a V) {
        match v {
            V::Ignored => (C::Ignored, &V::Ignored),
            _ => {
                // update per event
                // TODO use previously found v to update e
                let c = C::from(v);
                self.m.insert(c, v.clone());
                (c, v)
            }
        }
    }
}
impl ValueStore<C, V> for State {
    fn get(&self, c: &C) -> V {
        match self.m.get(c) {
            Some(v) => v.clone(),
            None => c.default_v(),
        }
    }
}
