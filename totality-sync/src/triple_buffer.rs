use std::{fmt::Debug, sync::{Arc, Mutex}};

#[allow(dead_code)]
use log::{trace, debug, info, warn, error};

pub fn buffer<T: Clone + Debug>(src: T) -> (ReadingView<T>, EditingView<T>) {
    TripleBuffer::alloc(src)
}

#[derive(Debug, Copy, Clone)]
struct TripleBufferIndices {
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
impl Default for TripleBufferIndices {
    fn default() -> Self {
        Self {
            snatched: 0,
            most_recent: 0,
            curr_write: 1,
            next_write: 2,
        }
    }
}

#[derive(Debug)]
struct TripleBuffer<T: Clone + Debug> {
    ii: Mutex<TripleBufferIndices>,
    backing_mem: *const [T; 3],
    tt: [*mut T; 3],
}
impl <T: Clone + Debug> TripleBuffer<T> {
    pub fn alloc(src: T) -> (ReadingView<T>, EditingView<T>) {
        let backing_mem: *mut [T; 3] = unsafe { Box::into_raw(Box::new(std::mem::uninitialized())) };
        let mut tt: [*mut T; 3] = unsafe { std::mem::uninitialized() };
        unsafe {
            for i in 0..2 {
                std::ptr::write(&mut (*backing_mem)[i], src.clone());
            }
            std::ptr::write(&mut (*backing_mem)[2], src.clone());
            for i in 0..3 {
                tt[i] = &mut (*backing_mem)[i];
            }
        }
        let arc = Arc::new(Self {
            ii: Mutex::new(TripleBufferIndices::default()),
            backing_mem,
            tt,
        });
        (ReadingView(arc.clone()), EditingView(arc))
    }
    fn snatch(&self) -> *const T {
        if let Ok(mut ii) = self.ii.lock() {
            self.tt[ii.snatch()]
        } else { panic!("Poisoned buffer indices!") }
    }
    fn advance(&self) -> (*const T, *mut T) {
        if let Ok(mut ii) = self.ii.lock() {
            let (i_read, i_write) = ii.advance();
            (self.tt[i_read], self.tt[i_write])
        } else { panic!("Poisoned buffer indices!") }
    }
}
impl <T: Clone + Debug> Drop for TripleBuffer<T> {
    fn drop(&mut self) {
        unsafe { Box::from_raw(self.backing_mem as *mut [T; 3]); };
    }
}

#[derive(Debug)]
pub struct RWPair<R, W> {
    r: R,
    w: W,
}

#[derive(Debug)]
pub enum Reading<T: Clone + Debug> {
    ReadingView(ReadingView<T>), Reader(Reader<T>),
}
#[derive(Debug)]
pub struct ReadingView<T: Clone + Debug>(Arc<TripleBuffer<T>>);
impl <T: Clone + Debug> ReadingView<T> {
    pub fn read(self) -> Reader<T> {
        Reader::from_view(self)
    }
}
unsafe impl <T: Clone + Debug> Send for ReadingView<T> {}
#[derive(Debug)]
pub struct Reader<T: Clone + Debug> {
    origin: ReadingView<T>,
    locker: *const T,
}
impl <T: Clone + Debug> Reader<T> {
    pub fn from_view(rv: ReadingView<T>) -> Reader<T> {
        Self {
            locker: rv.0.snatch(),
            origin: rv,
        }
    }
    pub fn r<'a>(&'a self) -> &'a T { unsafe { &*self.locker } }
    pub fn release(self) -> ReadingView<T> { self.origin }
}

#[derive(Debug)]
pub enum Editing<T: Clone + Debug> {
    EditingView(EditingView<T>), Editor(Editor<T>),
}
#[derive(Debug)]
pub struct EditingView<T: Clone + Debug>(Arc<TripleBuffer<T>>);
impl <T: Clone + Debug> EditingView<T> {
    pub fn edit(self) -> Editor<T> {
        Editor::from_view(self)
    }
}
unsafe impl <T: Clone + Debug> Send for EditingView<T> {}
#[derive(Debug)]
pub struct Editor<T: Clone + Debug> {
    origin: EditingView<T>,
    rw_lock: RWPair<*const T, *mut T>,
}
impl <T: Clone + Debug> Editor<T> {
    fn from_view(ev: EditingView<T>) -> Editor<T> {
        let (r, w) = ev.0.advance();
        Editor { origin: ev, rw_lock: RWPair { r: r, w: w } }
    }
    pub fn r<'a>(&'a self) -> &'a T { unsafe { & *self.rw_lock.r } }
    pub fn w<'a>(&'a self) -> &'a mut T { unsafe { &mut *self.rw_lock.w } }
    pub fn release(self) -> EditingView<T> { self.origin }
}
