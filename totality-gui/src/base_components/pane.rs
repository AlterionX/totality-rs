use crate::component::Component;
use crate::layout::Placer;

pub struct Pane<P: Placer> {
    children: Vec<Box<dyn Component>>,
    manager: P,
}
// impl <P: Placer> Component for Pane<P> {
//     fn sz(&self) -> Size {
//         Size(0, 0)
//     }
// }
// impl <P: Placer> RootComponent for Pane<P> {}
