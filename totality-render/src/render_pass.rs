// static commands
// data load
// semaphore acquisition
pub struct DrawPass {}

pub struct Node {
    // data: Vec<Id>,
// inputs: Vec<Id>,
}

// create graph
// trim graph
// record graph
// load data

pub trait DataLinkage<I: hal::Instance>: 'static + Send {
    fn next_req(&self) -> Option<super::RenderReq<I>>;
}
