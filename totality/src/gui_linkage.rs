//! Link together otherwise independent systems to the gui.

use gui::{draw::DrawCmd, linkage::{EventLinkage, DrawLinkage}};

pub struct EventSystemLinkage {
    // TODO register for needed events, possibly prevent other events/link to those systems as well
}
impl EventLinkage for EventSystemLinkage {
}
pub struct RenderSystemLinkage {
    // TODO update render state / cache, possibly prevent other events/link to those systems as well
}
impl DrawLinkage for RenderSystemLinkage {
    fn queue_cmd(&self, cmd: DrawCmd) {
    }
    fn disptach(&self) {
    }
}
