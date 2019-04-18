use std::{
    sync::mpsc::{channel, Receiver},
    cell::RefCell,
};
use winit::{
    Event, EventsLoop, ControlFlow, WindowEvent, DeviceEvent,
    KeyboardInput, VirtualKeyCode, ScanCode,
    WindowBuilder,
    dpi::*,
};
use super::*;

pub type Window = winit::Window;
pub struct IO {
    e_loop: Option<RefCell<EventsLoop>>,
}
impl IO {
    pub fn new() -> Self {
        Self {
            e_loop: Option::None,
        }
    }
}
impl super::IO for IO {
    type Window = winit::Window;
    type Event = winit::Event;
    fn init(&mut self) {
        self.e_loop.get_or_insert(RefCell::new(EventsLoop::new()));
    }
    fn next_events(&self, buf: &mut Vec<e::V>) {
        if let Some(ref e_loop) = self.e_loop {
            e_loop.borrow_mut().poll_events(|e| { buf.push(Self::to_v(e)) })
        }
    }
    fn create_window(&self, specs: WindowSpecs) -> Self::Window {
        if let Some(ref e_loop) = self.e_loop {
            WindowBuilder::new()
                .with_title(specs.name)
                // .with_dimensions()
                .build(&e_loop.borrow_mut())
                .expect("Fuck. Why can't I make a window?")
        } else {
            panic!("Init method must be called before creating a window.")
        }
    }
    fn to_v(e: Self::Event) -> e::V {
        let v = match e {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => e::b::V(e::b::Flag::Close.into(), e::b::State::DOWN).into(),
             Event::WindowEvent {
                 event: WindowEvent::Resized(LogicalSize { width, height }),
                 ..
             } => e::p::V::ScreenSz(e::p::SzState(na::Vector2::new(width as f32, height as f32))).into(),
             Event::WindowEvent {
                 event: WindowEvent::Refresh,
                 ..
             } =>  e::b::V(e::b::Flag::Refresh.into(), e::b::State::DOWN).into(),
             Event::WindowEvent {
                 event: WindowEvent::CursorEntered { .. },
                 ..
             } =>  e::b::V(e::b::Flag::CursorEntered.into(), e::b::State::DOWN).into(),
             Event::WindowEvent {
                 event: WindowEvent::CursorLeft { .. },
                 ..
             } =>  e::b::V(e::b::Flag::CursorEntered.into(), e::b::State::DOWN).into(),
             Event::WindowEvent {
                 event: WindowEvent::CursorMoved { position: p, .. },
                 ..
             } =>  e::p::V::CursorPos(e::p::PosState(as_vec(p))).into(),
             Event::WindowEvent {
                 event: WindowEvent::Focused(f),
                 ..
             } =>  e::b::V(e::b::Flag::Focus.into(), e::b::State::DOWN).into(),
             Event::DeviceEvent {
                 event: DeviceEvent::Key(k),
                 ..
             } => parse_keyboard(k),
             Event::WindowEvent {
                 event: WindowEvent::KeyboardInput {
                     input: k,
                     ..
                 },
                 ..
             } => parse_keyboard(k),
             Event::DeviceEvent {
                 event: DeviceEvent::Motion { .. },
                 ..
             } => e::V::Ignored,
             Event::DeviceEvent {
                 event: DeviceEvent::MouseMotion { delta: (x, y) },
                 ..
             } => e::p::V::MouseDelta(e::p::DeltaState(na::Vector2::new(x as f32, y as f32))).into(),
             Event::WindowEvent {
                 event: WindowEvent::AxisMotion {
                     ..
                 },
                 ..
             } => e::V::Ignored,
             Event::WindowEvent {
                 event: WindowEvent::ReceivedCharacter(_),
                 ..
             } => e::V::Ignored,
            _ => unimplemented!("Cannot cast {:?} to C.", e),
        };
        v
    }
}

fn parse_keyboard(k: KeyboardInput) -> e::V {
    if let Some(vk) = k.virtual_keycode {
        e::b::V(map_vk(vk), e::b::State::from(k.state == winit::ElementState::Pressed)).into()
    } else {
        // TODO e::b::V(map_sc(k.scancode), e::b::State::from(k.state == winit::ElementState::Pressed)).into()
        e::V::Ignored
    }
}

fn map_sc(sc: ScanCode) -> e::b::C {
    unimplemented!("Cannot parse scancode {:?} yet.", sc)
}
fn map_vk(vk: VirtualKeyCode) -> e::b::C {
    match vk {
        Key1 => e::b::C::A('1'),
        Key2 => e::b::C::A('2'),
        Key3 => e::b::C::A('3'),
        Key4 => e::b::C::A('4'),
        Key5 => e::b::C::A('5'),
        Key6 => e::b::C::A('6'),
        Key7 => e::b::C::A('7'),
        Key8 => e::b::C::A('8'),
        Key9 => e::b::C::A('9'),
        Key0 => e::b::C::A('0'),

        A => e::b::C::A('a'),
        B => e::b::C::A('b'),
        C => e::b::C::A('c'),
        D => e::b::C::A('d'),
        E => e::b::C::A('e'),
        F => e::b::C::A('f'),
        G => e::b::C::A('g'),
        H => e::b::C::A('h'),
        I => e::b::C::A('i'),
        J => e::b::C::A('j'),
        K => e::b::C::A('k'),
        L => e::b::C::A('l'),
        M => e::b::C::A('m'),
        N => e::b::C::A('n'),
        O => e::b::C::A('o'),
        P => e::b::C::A('p'),
        Q => e::b::C::A('q'),
        R => e::b::C::A('r'),
        S => e::b::C::A('s'),
        T => e::b::C::A('t'),
        U => e::b::C::A('u'),
        V => e::b::C::A('v'),
        W => e::b::C::A('w'),
        X => e::b::C::A('x'),
        Y => e::b::C::A('y'),
        Z => e::b::C::A('z'),

        Escape => e::b::Key::Esc.into(),

        F1 => e::b::Key::F(1).into(),
        F2 => e::b::Key::F(2).into(),
        F3 => e::b::Key::F(3).into(),
        F4 => e::b::Key::F(4).into(),
        F5 => e::b::Key::F(5).into(),
        F6 => e::b::Key::F(6).into(),
        F7 => e::b::Key::F(7).into(),
        F8 => e::b::Key::F(8).into(),
        F9 => e::b::Key::F(9).into(),
        F10 => e::b::Key::F(10).into(),
        F11 => e::b::Key::F(11).into(),
        F12 => e::b::Key::F(12).into(),
        F13 => e::b::Key::F(13).into(),
        F14 => e::b::Key::F(14).into(),
        F15 => e::b::Key::F(15).into(),
        F16 => e::b::Key::F(16).into(),
        F17 => e::b::Key::F(17).into(),
        F18 => e::b::Key::F(18).into(),
        F19 => e::b::Key::F(19).into(),
        F20 => e::b::Key::F(20).into(),
        F21 => e::b::Key::F(21).into(),
        F22 => e::b::Key::F(22).into(),
        F23 => e::b::Key::F(23).into(),
        F24 => e::b::Key::F(24).into(),

        Snapshot => e::b::Key::PrintScreen.into(),
        Scroll => e::b::Key::ScrLk.into(),
        Pause => e::b::Key::Pause.into(),

        Insert => e::b::Key::Ins.into(),
        Home => e::b::Key::Home.into(),
        Delete => e::b::Key::Del.into(),
        End => e::b::Key::End.into(),
        PageDown => e::b::Key::PgDn.into(),
        PageUp => e::b::Key::PgUp.into(),

        Left => e::b::Key::Left.into(),
        Up => e::b::Key::Up.into(),
        Right => e::b::Key::Right.into(),
        Down => e::b::Key::Down.into(),

        Back => e::b::Key::Backspace.into(),
        Return => e::b::Key::Enter.into(),
        Space => e::b::C::A(' '),

        // The "Compose" key on Linux.
        // Compose => e::b::Key::Compose.into(),

        Caret => e::b::C::A('^'),

        Numlock => e::b::Key::NumLk.into(),
        Numpad0 => e::b::C::A('0'),
        Numpad1 => e::b::C::A('1'),
        Numpad2 => e::b::C::A('2'),
        Numpad3 => e::b::C::A('3'),
        Numpad4 => e::b::C::A('4'),
        Numpad5 => e::b::C::A('5'),
        Numpad6 => e::b::C::A('6'),
        Numpad7 => e::b::C::A('7'),
        Numpad8 => e::b::C::A('8'),
        Numpad9 => e::b::C::A('9'),

        // AbntC1,
        // AbntC2,
        Add => e::b::C::A('+'),
        Apostrophe => e::b::C::A('\''),
        // Apps,
        At => e::b::C::A('@'),
        // Ax,
        Backslash => e::b::C::A('\\'),
        // Calculator,
        // Capital,
        Colon => e::b::C::A(':'),
        Comma => e::b::C::A(','),
        // Convert,
        Decimal => e::b::C::A('.'),
        Divide => e::b::C::A('/'),
        Equals => e::b::C::A('='),
        // Grave,
        // Kana,
        // Kanji,
        LAlt => e::b::Key::Alt(e::b::Side::L).into(),
        LBracket => e::b::C::A('['),
        LControl => e::b::Key::Alt(e::b::Side::L).into(),
        LShift => e::b::Key::Alt(e::b::Side::L).into(),
        LWin => e::b::Key::Alt(e::b::Side::L).into(),
        // Mail,
        // MediaSelect,
        // MediaStop,
        Minus => e::b::C::A('-'),
        Multiply => e::b::C::A('*'),
        // Mute,
        // MyComputer,
        // NavigateForward, // also called "Prior"
        // NavigateBackward, // also called "Next"
        // NextTrack,
        // NoConvert,
        NumpadComma => e::b::C::A(','),
        NumpadEnter => e::b::Key::Alt(e::b::Side::L).into(),
        NumpadEquals => e::b::C::A('='),
        // OEM102,
        Period => e::b::C::A('.'),
        // PlayPause,
        // Power,
        // PrevTrack,
        RAlt => e::b::Key::Alt(e::b::Side::R).into(),
        RBracket => e::b::C::A(']'),
        RControl => e::b::Key::Ctrl(e::b::Side::R).into(),
        RShift => e::b::Key::Shift(e::b::Side::R).into(),
        RWin => e::b::Key::Mod(e::b::Side::R).into(),
        Semicolon => e::b::C::A(';'),
        Slash => e::b::C::A('/'),
        // Sleep,
        // Stop,
        Subtract => e::b::C::A('-'),
        // Sysrq,
        Tab => e::b::C::A('\t'),
        // Underline,
        // Unlabeled,
        // VolumeDown,
        // VolumeUp,
        // Wake,
        // WebBack,
        // WebFavorites,
        // WebForward,
        // WebHome,
        // WebRefresh,
        // WebSearch,
        // WebStop,
        // Yen,
        // Copy,
        // Paste,
        // Cut,
    }
}
fn as_vec(p: LogicalPosition) -> na::Vector2<f32> {
    na::Vector2::new(p.x as f32, p.y as f32)
}

