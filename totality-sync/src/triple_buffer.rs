use std::sync::{Arc, RwLock, Mutex};

#[allow(dead_code)]
use log::{trace, debug, info, warn, error};

#[derive(Debug, Copy, Clone)]
pub struct TripleBufferIndices {
    snatched: usize,
    most_recent: usize,
    curr_write: usize,
    next_write: usize,
}
impl TripleBufferIndices {
    fn snatch(&mut self) -> usize {
        trace!("Snatching indices: {:?}.", self);
        if self.snatched != self.most_recent {
            self.next_write = self.snatched;
            self.snatched = self.most_recent;
        }
        trace!("Reached indices: {:?}.", self);
        return self.snatched;
    }
    fn advance(&mut self) -> (usize, usize) {
        trace!("Advancing indices: {:?}.", self);
        self.most_recent = self.curr_write;
        self.curr_write = self.next_write;
        self.next_write = self.most_recent;
        trace!("Reached indices: {:?}.", self);
        return (self.most_recent, self.curr_write);
    }
}

#[derive(Debug)]
pub struct TripleBuffer<T: Clone> {
    ii: Mutex<TripleBufferIndices>,
    tt: [Arc<RwLock<T>>; 3],
}
impl <T: Clone> TripleBuffer<T> {
    pub fn new(t: T) -> Self { Self {
        ii: Mutex::new(TripleBufferIndices {
            snatched: 0usize,
            most_recent: 0usize,
            curr_write: 1usize,
            next_write: 2usize,
        }),
        tt: [
            Arc::new(RwLock::new(t.clone())),
            Arc::new(RwLock::new(t.clone())),
            Arc::new(RwLock::new(t)), // move is intentional
        ],
    } }
    pub fn snatch(&self) -> Arc<RwLock<T>> {
        if let Ok(mut ii) = self.ii.lock() {
            self.tt[ii.snatch()].clone()
        } else { panic!("Poisoned buffer indices!") }
    }
    pub fn advance(&self) -> (Arc<RwLock<T>>, Arc<RwLock<T>>) {
        if let Ok(mut ii) = self.ii.lock() {
            let (ii_read, ii_write) = ii.advance();
            (self.tt[ii_read].clone(), self.tt[ii_write].clone())
        } else { panic!("Poisoned buffer indices!") }
    }
}
