#[derive(Debug, Copy, Clone)]
pub enum ChV<T> {
    Any,
    D(T),
}
impl<T> ChV<T> {
    fn satisfied_by<U>(&self, other: &ChV<U>) -> bool
    where
        T: PartialEq<U>,
    {
        match (self, other) {
            (ChV::Any, _) => true,
            (ChV::D(lhs), ChV::D(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}
impl<T> From<T> for ChV<T> {
    fn from(t: T) -> ChV<T> {
        ChV::D(t)
    }
}
impl<T> From<Option<T>> for ChV<T> {
    fn from(opt_t: Option<T>) -> ChV<T> {
        match opt_t {
            Some(t) => ChV::D(t),
            None => ChV::Any,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Ch<T> {
    before: ChV<T>,
    after: ChV<T>,
}
impl<T> Ch<T> {
    pub fn new<U, V>(before: U, after: V) -> Ch<T>
    where
        U: Into<ChV<T>>,
        V: Into<ChV<T>>,
    {
        Ch {
            before: before.into(),
            after: after.into(),
        }
    }
    pub fn any() -> Ch<T> {
        Ch {
            before: ChV::Any,
            after: ChV::Any,
        }
    }
    pub fn to<U>(after: U) -> Ch<T>
    where
        U: Into<ChV<T>>,
    {
        Ch {
            before: ChV::Any,
            after: after.into(),
        }
    }
    pub fn from<U>(before: T) -> Ch<T>
    where
        U: Into<ChV<T>>,
    {
        Ch {
            before: before.into(),
            after: ChV::Any,
        }
    }
    pub fn satisfied_by<U>(&self, other: &Ch<U>) -> bool
    where
        T: PartialEq<U>,
    {
        self.before.satisfied_by(&other.before) && self.after.satisfied_by(&other.after)
    }
}
