extern crate totality_events as ext_events;

use ext_events::cb;
use ext_events::hal as e;

pub type CBFn = ();
pub type CB = ();

pub enum E {
    Hover,
    Click,
    Dropped(String),
    Scroll,
    Key(e::b::Key),
}
