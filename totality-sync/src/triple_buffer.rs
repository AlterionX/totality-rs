use std::{
    cell::UnsafeCell,
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc,
    },
};

use cb::utils::CachePadded;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

pub fn buffer<T: Clone>(src: T) -> (ReadingView<T>, EditingView<T>) {
    TripleBuffer::alloc(src)
}

#[derive(Debug)]
struct TripleBufferIndices {
    snatched_read: CachePadded<usize>,     // unique
    packed_vals: CachePadded<AtomicUsize>, // shared
    stale: CachePadded<AtomicBool>,        // shared
    edit_rw: CachePadded<(usize, usize)>,  // unique
}
impl TripleBufferIndices {
    #[inline]
    fn pack(v0: usize, v1: usize) -> usize {
        (0b0 << 4) + (v0 << 2) + ((!v1) & 0b11)
    }
    #[inline]
    fn unpack(packed: usize) -> (usize, usize) {
        let should_negate = ((packed >> 4) & 0b1) != 0;
        let most_recent = (if should_negate { !packed } else { packed } >> 2) & 0b11;
        let next_write = !packed & 0b11;
        (most_recent, next_write)
    }
    fn snatch(&mut self) {
        let mask = (0b1 << 4)
            + (0b11 << 2)
            + match *self.snatched_read {
                0 => 0b00,
                1 => 0b01,
                2 => 0b10,
                _ => panic!("We done goofed!"),
            };
        let old_snatched = self.snatched_read;
        if !self.stale.swap(true, std::sync::atomic::Ordering::Acquire) {
            *self.snatched_read = Self::unpack(
                self.packed_vals
                    .fetch_nand(mask, std::sync::atomic::Ordering::AcqRel),
            )
            .0;
            trace!(
                "Snatching indices {:?} and returning indices {:?}.",
                old_snatched,
                self.snatched_read
            );
        }
    }
    fn advance(&mut self) {
        let next_write = Self::unpack(self.packed_vals.swap(
            Self::pack(self.edit_rw.1, self.edit_rw.1),
            std::sync::atomic::Ordering::AcqRel,
        ))
        .1;
        self.stale.swap(false, std::sync::atomic::Ordering::Release);
        trace!(
            "Advancing indices from {:?} to {:?}.",
            self.edit_rw.1,
            next_write
        );
        self.edit_rw.0 = self.edit_rw.1;
        self.edit_rw.1 = next_write;
    }
}
impl Default for TripleBufferIndices {
    fn default() -> Self {
        Self {
            snatched_read: CachePadded::new(0),
            packed_vals: CachePadded::new(AtomicUsize::new(Self::pack(0, 2))),
            stale: CachePadded::new(AtomicBool::new(true)),
            edit_rw: CachePadded::new((1, 2)),
        }
    }
}

struct TripleBuffer<T: Clone> {
    ii: UnsafeCell<TripleBufferIndices>,
    backing_mem: *const [UnsafeCell<CachePadded<T>>; 3],
    tt: [*mut T; 3],
}
impl<T: Clone> TripleBuffer<T> {
    pub fn alloc(src: T) -> (ReadingView<T>, EditingView<T>) {
        let backing_mem = Box::into_raw(Box::new([
            UnsafeCell::new(CachePadded::new(src.clone())),
            UnsafeCell::new(CachePadded::new(src.clone())),
            UnsafeCell::new(CachePadded::new(src)),
        ]));
        let mut tt: [*mut T; 3] = unsafe { std::mem::uninitialized() };
        unsafe {
            for i in 0..3 {
                tt[i] = &mut **(*backing_mem)[i].get();
            }
        }
        let arc = Arc::new(Self {
            ii: UnsafeCell::new(TripleBufferIndices::default()),
            backing_mem,
            tt,
        });
        (ReadingView(arc.clone()), EditingView(arc))
    }
    fn snatch(&self) {
        let ii = self.ii.get();
        unsafe { (*ii).snatch() };
    }
    fn advance(&self) {
        let ii = self.ii.get();
        unsafe { (*ii).advance() };
    }
    fn rr(&self) -> *const T {
        let ii = self.ii.get();
        self.tt[unsafe { *(*ii).snatched_read }]
    }
    fn er(&self) -> *const T {
        let ii = self.ii.get();
        self.tt[unsafe { (*ii).edit_rw.0 }]
    }
    fn ew(&self) -> *mut T {
        let ii = self.ii.get();
        self.tt[unsafe { (*ii).edit_rw.1 }]
    }
}
impl<T: Clone> Drop for TripleBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.backing_mem as *mut [CachePadded<T>; 3]);
        };
    }
}

pub struct RWPair<R, W> {
    r: R,
    w: W,
}

pub enum Reading<T: Clone> {
    ReadingView(ReadingView<T>),
    Reader(Reader<T>),
}
pub struct ReadingView<T: Clone>(Arc<TripleBuffer<T>>);
impl<T: Clone> ReadingView<T> {
    pub fn read(self) -> Reader<T> {
        Reader::from_view(self)
    }
}
unsafe impl<T: Clone> Send for ReadingView<T> {}
pub struct Reader<T: Clone> {
    origin: ReadingView<T>,
    locker: *const T,
}
impl<T: Clone> Reader<T> {
    pub fn from_view(rv: ReadingView<T>) -> Reader<T> {
        rv.0.snatch();
        Self {
            locker: rv.0.rr(),
            origin: rv,
        }
    }
    pub fn r<'a>(&'a self) -> &'a T {
        unsafe { &*self.locker }
    }
    pub fn release(self) -> ReadingView<T> {
        self.origin
    }
}

pub enum Editing<T: Clone> {
    EditingView(EditingView<T>),
    Editor(Editor<T>),
}
pub struct EditingView<T: Clone>(Arc<TripleBuffer<T>>);
impl<T: Clone> EditingView<T> {
    pub fn edit(self) -> Editor<T> {
        Editor::from_view(self)
    }
}
unsafe impl<T: Clone> Send for EditingView<T> {}
pub struct Editor<T: Clone> {
    origin: EditingView<T>,
    rw_lock: RWPair<*const T, *mut T>,
}
impl<T: Clone> Editor<T> {
    fn from_view(ev: EditingView<T>) -> Editor<T> {
        Editor {
            rw_lock: RWPair {
                r: ev.0.er(),
                w: ev.0.ew(),
            },
            origin: ev,
        }
    }
    pub fn r<'a>(&'a self) -> &'a T {
        unsafe { &*self.rw_lock.r }
    }
    pub fn w<'a>(&'a self) -> &'a mut T {
        unsafe { &mut *self.rw_lock.w }
    }
    pub fn release(self) -> EditingView<T> {
        self.origin.0.advance();
        self.origin
    }
}
