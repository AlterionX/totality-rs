use crate::draw;

pub trait DrawLinkage {
    fn queue_cmd(&self, cmd: draw::Cmd);
    fn disptach(&self);
}
pub trait EventLinkage {}
