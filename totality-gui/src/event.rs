extern crate totality_hal_events as ext_events;

use ext_events as e;

pub enum E {
    Hover, Click, Dropped(String), Scroll, Key(e::b::Key)
}
