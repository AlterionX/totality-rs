use std::cell::RefCell;

use super::WindowSpecs;
use internal_events::hal as e;

use log::trace;
use winit::{
    dpi::*, DeviceEvent, Event, EventsLoop, KeyboardInput, VirtualKeyCode,
    WindowBuilder, WindowEvent,
};

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
            e_loop.borrow_mut().poll_events(|e| buf.push(Self::to_v(e)))
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
        trace!("Processing Event {:?}", e);
        let v = match e {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => e::b::V(e::b::Flag::Close.into(), e::b::State::DOWN).into(),
            Event::WindowEvent {
                event: WindowEvent::Resized(LogicalSize { width, height }),
                ..
            } => e::p::V::ScreenSz(e::p::SzState(na::Vector2::new(width as f32, height as f32)))
                .into(),
            Event::WindowEvent {
                event: WindowEvent::Refresh,
                ..
            } => e::b::V(e::b::Flag::Refresh.into(), e::b::State::DOWN).into(),
            Event::WindowEvent {
                event: WindowEvent::CursorEntered { .. },
                ..
            } => e::b::V(e::b::Flag::CursorEntered.into(), e::b::State::DOWN).into(),
            Event::WindowEvent {
                event: WindowEvent::CursorLeft { .. },
                ..
            } => e::b::V(e::b::Flag::CursorEntered.into(), e::b::State::DOWN).into(),
            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position: p, .. },
                ..
            } => e::p::V::CursorPos(e::p::PosState(as_vec(p))).into(),
            Event::WindowEvent {
                event: WindowEvent::Focused(_),
                ..
            } => e::b::V(e::b::Flag::Focus.into(), e::b::State::DOWN).into(),
            Event::DeviceEvent {
                // Ignored since it's actually a duplicate of the below.
                event: DeviceEvent::Key(_),
                ..
            } => e::V::Ignored,
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input: k, .. },
                ..
            } => parse_keyboard(k),
            Event::DeviceEvent {
                event: DeviceEvent::Motion { .. },
                ..
            } => e::V::Ignored,
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta: _ },
                ..
            } => e::V::Ignored,
            Event::WindowEvent {
                event: WindowEvent::AxisMotion { .. },
                ..
            } => e::V::Ignored,
            Event::WindowEvent {
                event: WindowEvent::ReceivedCharacter(_),
                ..
            } => e::V::Ignored,
            Event::Awakened => e::V::Ignored,
            _ => unimplemented!("Cannot cast {:?} to C.", e),
        };
        trace!("Event {:?} translated to {:?}", e, v);
        v
    }
}

fn parse_keyboard(k: KeyboardInput) -> e::V {
    if let Some(vk) = k.virtual_keycode {
        match map_vk(vk) {
            e::b::C::Ignored => e::V::Ignored,
            c @ _ => e::b::V(
                c,
                e::b::State::from(k.state == winit::ElementState::Pressed),
            )
            .into(),
        }
    } else {
        // TODO e::b::V(map_sc(k.scancode), e::b::State::from(k.state == winit::ElementState::Pressed)).into()
        e::V::Ignored
    }
}

fn map_vk(vk: VirtualKeyCode) -> e::b::C {
    match vk {
        VirtualKeyCode::Key1 => '1'.into(),
        VirtualKeyCode::Key2 => '2'.into(),
        VirtualKeyCode::Key3 => '3'.into(),
        VirtualKeyCode::Key4 => '4'.into(),
        VirtualKeyCode::Key5 => '5'.into(),
        VirtualKeyCode::Key6 => '6'.into(),
        VirtualKeyCode::Key7 => '7'.into(),
        VirtualKeyCode::Key8 => '8'.into(),
        VirtualKeyCode::Key9 => '9'.into(),
        VirtualKeyCode::Key0 => '0'.into(),

        VirtualKeyCode::A => 'a'.into(),
        VirtualKeyCode::B => 'b'.into(),
        VirtualKeyCode::C => 'c'.into(),
        VirtualKeyCode::D => 'd'.into(),
        VirtualKeyCode::E => 'e'.into(),
        VirtualKeyCode::F => 'f'.into(),
        VirtualKeyCode::G => 'g'.into(),
        VirtualKeyCode::H => 'h'.into(),
        VirtualKeyCode::I => 'i'.into(),
        VirtualKeyCode::J => 'j'.into(),
        VirtualKeyCode::K => 'k'.into(),
        VirtualKeyCode::L => 'l'.into(),
        VirtualKeyCode::M => 'm'.into(),
        VirtualKeyCode::N => 'n'.into(),
        VirtualKeyCode::O => 'o'.into(),
        VirtualKeyCode::P => 'p'.into(),
        VirtualKeyCode::Q => 'q'.into(),
        VirtualKeyCode::R => 'r'.into(),
        VirtualKeyCode::S => 's'.into(),
        VirtualKeyCode::T => 't'.into(),
        VirtualKeyCode::U => 'u'.into(),
        VirtualKeyCode::V => 'v'.into(),
        VirtualKeyCode::W => 'w'.into(),
        VirtualKeyCode::X => 'x'.into(),
        VirtualKeyCode::Y => 'y'.into(),
        VirtualKeyCode::Z => 'z'.into(),

        VirtualKeyCode::Escape => e::b::Key::Esc.into(),

        VirtualKeyCode::F1 => e::b::Key::F(1).into(),
        VirtualKeyCode::F2 => e::b::Key::F(2).into(),
        VirtualKeyCode::F3 => e::b::Key::F(3).into(),
        VirtualKeyCode::F4 => e::b::Key::F(4).into(),
        VirtualKeyCode::F5 => e::b::Key::F(5).into(),
        VirtualKeyCode::F6 => e::b::Key::F(6).into(),
        VirtualKeyCode::F7 => e::b::Key::F(7).into(),
        VirtualKeyCode::F8 => e::b::Key::F(8).into(),
        VirtualKeyCode::F9 => e::b::Key::F(9).into(),
        VirtualKeyCode::F10 => e::b::Key::F(10).into(),
        VirtualKeyCode::F11 => e::b::Key::F(11).into(),
        VirtualKeyCode::F12 => e::b::Key::F(12).into(),
        VirtualKeyCode::F13 => e::b::Key::F(13).into(),
        VirtualKeyCode::F14 => e::b::Key::F(14).into(),
        VirtualKeyCode::F15 => e::b::Key::F(15).into(),
        VirtualKeyCode::F16 => e::b::Key::F(16).into(),
        VirtualKeyCode::F17 => e::b::Key::F(17).into(),
        VirtualKeyCode::F18 => e::b::Key::F(18).into(),
        VirtualKeyCode::F19 => e::b::Key::F(19).into(),
        VirtualKeyCode::F20 => e::b::Key::F(20).into(),
        VirtualKeyCode::F21 => e::b::Key::F(21).into(),
        VirtualKeyCode::F22 => e::b::Key::F(22).into(),
        VirtualKeyCode::F23 => e::b::Key::F(23).into(),
        VirtualKeyCode::F24 => e::b::Key::F(24).into(),

        VirtualKeyCode::Snapshot => e::b::Key::PrintScreen.into(),
        VirtualKeyCode::Scroll => e::b::Key::ScrLk.into(),
        VirtualKeyCode::Pause => e::b::Key::Pause.into(),

        VirtualKeyCode::Insert => e::b::Key::Ins.into(),
        VirtualKeyCode::Home => e::b::Key::Home.into(),
        VirtualKeyCode::Delete => e::b::Key::Del.into(),
        VirtualKeyCode::End => e::b::Key::End.into(),
        VirtualKeyCode::PageDown => e::b::Key::PgDn.into(),
        VirtualKeyCode::PageUp => e::b::Key::PgUp.into(),

        VirtualKeyCode::Left => e::b::Key::Left.into(),
        VirtualKeyCode::Up => e::b::Key::Up.into(),
        VirtualKeyCode::Right => e::b::Key::Right.into(),
        VirtualKeyCode::Down => e::b::Key::Down.into(),

        VirtualKeyCode::Back => e::b::Key::Backspace.into(),
        VirtualKeyCode::Return => e::b::Key::Enter.into(),
        VirtualKeyCode::Space => e::b::C::A(' '),

        // The "Compose" key on Linux.
        // VirtualKeyCode::Compose => e::b::Key::Compose.into(),
        VirtualKeyCode::Caret => '^'.into(),

        VirtualKeyCode::Numlock => e::b::Key::NumLk.into(),
        VirtualKeyCode::Numpad0 => '0'.into(),
        VirtualKeyCode::Numpad1 => '1'.into(),
        VirtualKeyCode::Numpad2 => '2'.into(),
        VirtualKeyCode::Numpad3 => '3'.into(),
        VirtualKeyCode::Numpad4 => '4'.into(),
        VirtualKeyCode::Numpad5 => '5'.into(),
        VirtualKeyCode::Numpad6 => '6'.into(),
        VirtualKeyCode::Numpad7 => '7'.into(),
        VirtualKeyCode::Numpad8 => '8'.into(),
        VirtualKeyCode::Numpad9 => '9'.into(),

        // VirtualKeyCode::AbntC1,
        // VirtualKeyCode::AbntC2,
        VirtualKeyCode::Add => e::b::C::A('+'),
        VirtualKeyCode::Apostrophe => e::b::C::A('\''),
        // VirtualKeyCode::Apps,
        VirtualKeyCode::At => e::b::C::A('@'),
        // VirtualKeyCode::Ax,
        VirtualKeyCode::Backslash => e::b::C::A('\\'),
        // VirtualKeyCode::Calculator,
        // VirtualKeyCode::Capital,
        VirtualKeyCode::Colon => e::b::C::A(':'),
        VirtualKeyCode::Comma => e::b::C::A(','),
        // VirtualKeyCode::Convert,
        VirtualKeyCode::Decimal => e::b::C::A('.'),
        VirtualKeyCode::Divide => e::b::C::A('/'),
        VirtualKeyCode::Equals => e::b::C::A('='),
        // VirtualKeyCode::Grave,
        // VirtualKeyCode::Kana,
        // VirtualKeyCode::Kanji,
        VirtualKeyCode::LAlt => e::b::Key::Alt(e::b::Side::L).into(),
        VirtualKeyCode::LBracket => e::b::C::A('['),
        VirtualKeyCode::LControl => e::b::Key::Alt(e::b::Side::L).into(),
        VirtualKeyCode::LShift => e::b::Key::Alt(e::b::Side::L).into(),
        VirtualKeyCode::LWin => e::b::Key::Alt(e::b::Side::L).into(),
        // VirtualKeyCode::Mail,
        // VirtualKeyCode::MediaSelect,
        // VirtualKeyCode::MediaStop,
        VirtualKeyCode::Minus => e::b::C::A('-'),
        VirtualKeyCode::Multiply => e::b::C::A('*'),
        // VirtualKeyCode::Mute,
        // VirtualKeyCode::MyComputer,
        // VirtualKeyCode::NavigateForward, // also called "Prior"
        // VirtualKeyCode::NavigateBackward, // also called "Next"
        // VirtualKeyCode::NextTrack,
        // VirtualKeyCode::NoConvert,
        VirtualKeyCode::NumpadComma => e::b::C::A(','),
        VirtualKeyCode::NumpadEnter => e::b::Key::Alt(e::b::Side::L).into(),
        VirtualKeyCode::NumpadEquals => e::b::C::A('='),
        // VirtualKeyCode::OEM102,
        VirtualKeyCode::Period => e::b::C::A('.'),
        // VirtualKeyCode::PlayPause,
        // VirtualKeyCode::Power,
        // VirtualKeyCode::PrevTrack,
        VirtualKeyCode::RAlt => e::b::Key::Alt(e::b::Side::R).into(),
        VirtualKeyCode::RBracket => e::b::C::A(']'),
        VirtualKeyCode::RControl => e::b::Key::Ctrl(e::b::Side::R).into(),
        VirtualKeyCode::RShift => e::b::Key::Shift(e::b::Side::R).into(),
        VirtualKeyCode::RWin => e::b::Key::Mod(e::b::Side::R).into(),
        VirtualKeyCode::Semicolon => e::b::C::A(';'),
        VirtualKeyCode::Slash => e::b::C::A('/'),
        // VirtualKeyCode::Sleep,
        // VirtualKeyCode::Stop,
        VirtualKeyCode::Subtract => e::b::C::A('-'),
        // VirtualKeyCode::Sysrq,
        VirtualKeyCode::Tab => e::b::C::A('\t'),
        // VirtualKeyCode::Underline,
        // VirtualKeyCode::Unlabeled,
        // VirtualKeyCode::VolumeDown,
        // VirtualKeyCode::VolumeUp,
        // VirtualKeyCode::Wake,
        // VirtualKeyCode::WebBack,
        // VirtualKeyCode::WebFavorites,
        // VirtualKeyCode::WebForward,
        // VirtualKeyCode::WebHome,
        // VirtualKeyCode::WebRefresh,
        // VirtualKeyCode::WebSearch,
        // VirtualKeyCode::WebStop,
        // VirtualKeyCode::Yen,
        // VirtualKeyCode::Copy,
        // VirtualKeyCode::Paste,
        // VirtualKeyCode::Cut,
        _ => e::b::C::Ignored.into(),
    }
}
fn as_vec(p: LogicalPosition) -> na::Vector2<f32> {
    na::Vector2::new(p.x as f32, p.y as f32)
}
