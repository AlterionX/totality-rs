#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum C {
    A(char), // Alpha-numeric + punctuation
    S(Key), // special, like ESC, ALT, SHIFT, etc.
    F(Flag), // flag, like window close
}
impl C {
    fn default_value(&self) -> V { V::default_value_of(&self) }
}
impl From<V> for C {
    fn from(v: V) -> C { v.0 }
}

// TODO look into distinctions between left and right ctrl, shift, alt, etc
#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum Key {
    Esc,
    Alt(Side),
    Shift(Side),
    Ctrl(Side),
    Mod(Side),
    Tab,
    Home, End, PgDn, PgUp, Ins, Del,
    Enter, Backspace,
    Up, Left, Down, Right,
    NumLk, ScrLk, CapLk,
    PrintScreen, Pause,
    F(u32/*TODO make this ranged*/),
}
impl From<Key> for C {
    fn from(k: Key) -> C { C::S(k) }
}
#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum Side { L, R }

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum Flag {
    Close,
    CursorEntered,
    Refresh,
    Focus,
}
impl From<Flag> for C {
    fn from(f: Flag) -> C { C::F(f) }
}

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub enum State {
    UP,
    DOWN,
}
impl From<bool> for State {
    fn from(b: bool) -> State {
        if b { State::UP } else { State::DOWN }
    }
}
impl From<State> for bool {
    fn from(s: State) -> bool {
        match s {
            UP => false,
            DOWN => true,
        }
    }
}

#[derive(Debug, Hash, Copy, Clone, PartialEq, Eq)]
pub struct V(pub C, pub State);
impl V {
    pub fn value(&self) -> State { self.1.clone() }
    pub fn default_value_of(c: &C) -> Self { Self(c.clone(), State::UP) }
}
impl From<(C, State)> for V {
    fn from((a, b): (C, State)) -> V { V(a, b) }
}

