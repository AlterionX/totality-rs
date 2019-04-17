use na::Vector2;

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum C {
    MousePos, MouseDelta, ScreenPos, CursorPos, ScreenSz,
}

#[derive(Debug, Copy, Clone)]
pub struct PosState(pub Vector2<f32>);
#[derive(Debug, Copy, Clone)]
pub struct SzState(pub Vector2<f32>);
#[derive(Debug, Copy, Clone)]
pub struct DeltaState(pub Vector2<f32>);

#[derive(Debug, Copy, Clone)]
pub enum V {
    MousePos(PosState),
    MouseDelta(DeltaState),
    ScreenPos(PosState),
    ScreenSz(SzState),
    CursorPos(PosState),
}
impl V {
    pub fn default_value_of(c: &C) -> Self {
        match c {
            MousePos => V::MousePos(PosState(Vector2::new(0f32, 0f32))),
            MouseDelta => V::MouseDelta(DeltaState(Vector2::new(0f32, 0f32))),
            ScreenPos => V::ScreenPos(PosState(Vector2::new(0f32, 0f32))),
            ScreenSz => V::ScreenPos(PosState(Vector2::new(0f32, 0f32))),
            CursorPos => V::ScreenPos(PosState(Vector2::new(0f32, 0f32))),
        }
    }
}
impl From<V> for C {
    fn from(v: V) -> C {
        match v {
            V::MousePos(_) => C::MousePos,
            V::MouseDelta(_) => C::MouseDelta,
            V::ScreenPos(_) => C::ScreenPos,
            V::ScreenSz(_) => C::ScreenSz,
            V::CursorPos(_) => C::CursorPos,
        }
    }
}

