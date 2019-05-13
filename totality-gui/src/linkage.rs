use crate::draw::DrawCmd;

pub trait DrawLinkage {
    fn queue_cmd(&self, cmd: DrawCmd);
    fn disptach(&self);
}
pub trait EventLinkage {
}
