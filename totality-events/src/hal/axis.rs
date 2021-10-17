#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum C {
    Scroll,
}
impl C {
    pub fn default_value(&self) -> V {
        V::default_value_of(self)
    }
}
impl From<&V> for C {
    fn from(v: &V) -> C {
        match v {
            V::Scroll(_) => C::Scroll,
        }
    }
}
impl From<V> for C {
    fn from(v: V) -> C {
        match v {
            V::Scroll(_) => C::Scroll,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum V {
    Scroll(f32),
}
impl V {
    pub fn default_value_of(c: &C) -> Self {
        match c {
            C::Scroll => V::Scroll(0f32),
        }
    }
}
