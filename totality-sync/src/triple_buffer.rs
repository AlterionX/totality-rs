use std::{
    cell::UnsafeCell,
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    marker::Send,
};

use cb::utils::CachePadded;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

pub fn buffer<T:Clone>(src: T) -> (Reader<T>, Editor<T>) {
    TripleBuffer::alloc(src)
}

#[derive(Debug)]
struct TripleBufferIndices {
    snatched_read: CachePadded<u8>,     // unique
    packed: CachePadded<AtomicU8>, // shared
    edit_rw: CachePadded<(u8, u8)>,  // unique
}
impl TripleBufferIndices {
    const BUFFER_ID_MASK: u8 = 0b11;
    const IS_NEW_MASK: u8 = 0b100;
    #[inline]
    fn pack(is_new: bool, v: u8) -> u8 {
        if is_new {
            v | 0b100
        } else {
            v
        }
    }
    #[inline]
    fn unpack(packed: u8) -> (bool, u8) {
        let is_new = (packed & Self::IS_NEW_MASK) != 0;
        let next_write = packed & Self::BUFFER_ID_MASK;
        (is_new, next_write)
    }
    fn snatch(&mut self) {
        let old_snatched = self.snatched_read;
        if Self::unpack(self.packed.load(Ordering::Acquire)).0 {
            *self.snatched_read = Self::unpack(
                self.packed.swap(Self::pack(true, *old_snatched), Ordering::AcqRel),
            )
            .1;
            trace!(
                "Snatching indices {:?} and returning indices {:?}.",
                old_snatched,
                self.snatched_read
            );
        }
    }
    fn advance(&mut self) {
        let curr_read = self.edit_rw.1;
        let curr_write = Self::unpack(self.packed.swap(
            Self::pack(true, curr_read),
            std::sync::atomic::Ordering::AcqRel,
        ))
        .1;
        trace!(
            "Advancing indices from {:?} to {:?}.",
            curr_read,
            curr_write
        );
        self.edit_rw.0 = curr_read;
        self.edit_rw.1 = curr_write;
    }
}
impl Default for TripleBufferIndices {
    fn default() -> Self {
        Self {
            snatched_read: CachePadded::new(0),
            packed: CachePadded::new(AtomicU8::new(Self::pack(false, 2))),
            edit_rw: CachePadded::new((1, 2)),
        }
    }
}

#[derive(Debug)]
struct TripleBuffer<T:Clone> {
    ii: UnsafeCell<TripleBufferIndices>,
    backing_mem: *const [UnsafeCell<CachePadded<T>>; 3],
    tt: [*mut T; 3],
}
impl<T:Clone> TripleBuffer<T> {
    pub fn alloc(src: T) -> (Reader<T>, Editor<T>) {
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
        (Reader::Free(TB(arc.clone())), Editor::Free(TB(arc)))
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
        self.tt[unsafe { *(*ii).snatched_read as usize }]
    }
    fn er(&self) -> *const T {
        let ii = self.ii.get();
        self.tt[unsafe { (*ii).edit_rw.0 as usize }]
    }
    fn ew(&self) -> *mut T {
        let ii = self.ii.get();
        self.tt[unsafe { (*ii).edit_rw.1 as usize }]
    }
}
impl<T:Clone> Drop for TripleBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.backing_mem as *mut [CachePadded<T>; 3]);
        };
    }
}
unsafe impl<T: Clone + Send> Send for TripleBuffer<T> {}
unsafe impl<T: Clone + Sync> Sync for TripleBuffer<T> {}

#[derive(Debug)]
pub struct TB<T:Clone>(Arc<TripleBuffer<T>>);
#[derive(Debug)]
pub struct LockedError<T>(pub T);
#[derive(Debug)]
pub struct UnlockedError<T>(pub T);

#[derive(Debug)]
pub enum Reader<T:Clone> {
    Free(TB<T>),
    Locked(LockedReader<T>),
}
impl<T:Clone> Reader<T> {
    #[must_use]
    pub fn grab(self) -> Result<Self, LockedError<Self>> {
        match self {
            Reader::Free(tb) => Ok(Reader::Locked(LockedReader::lock(tb))),
            _ => Err(LockedError(self)), // TODO consider returning self
        }
    }
    #[must_use]
    pub fn fetch<'a>(&'a self) -> Result<&'a T, UnlockedError<()>> {
        match self {
            Reader::Locked(lr) => Ok(lr.r()),
            _ => Err(UnlockedError(())), // TODO consider returning self
        }
    }
    pub fn fetch_unsafe<'a>(&'a self) -> &'a T {
        self.fetch().expect("Attempted to get buffers from old buffer.")
    }
    #[must_use]
    pub fn release(self) -> Result<Self, UnlockedError<Self>> {
        match self {
            Reader::Locked(lr) => Ok(Reader::Free(lr.release())),
            _ => Err(UnlockedError(self)), // TODO consider returning self
        }
    }
}
#[derive(Debug)]
pub struct LockedReader<T:Clone> {
    origin: Arc<TripleBuffer<T>>,
    locked: *const T,
}
impl<T:Clone> LockedReader<T> {
    fn lock(rv: TB<T>) -> LockedReader<T> {
        rv.0.snatch();
        Self {
            locked: rv.0.rr(),
            origin: rv.0,
        }
    }
    pub fn r<'a>(&'a self) -> &'a T {
        unsafe { &*self.locked }
    }
    fn release(self) -> TB<T> {
        TB(self.origin)
    }
}
unsafe impl<T: Clone + Send> Send for LockedReader<T> {}
unsafe impl<T: Clone + Sync> Sync for LockedReader<T> {}

#[derive(Debug)]
pub struct RWPair<R, W> {
    pub r: R,
    pub w: W,
}
#[derive(Debug)]
pub enum Editor<T:Clone> {
    Free(TB<T>),
    Locked(LockedEditor<T>),
}
impl<T:Clone> Editor<T> {
    #[must_use]
    pub fn grab(self) -> Result<Self, LockedError<Self>> {
        match self {
            Editor::Free(tb) => Ok(Editor::Locked(LockedEditor::lock(tb.0))),
            _ => Err(LockedError(self)),
        }
    }
    #[must_use]
    pub fn fetch<'a>(&'a self) -> Result<RWPair<&'a T, &'a mut T>, UnlockedError<()>> {
        match self {
            Editor::Locked(lr) => Ok(RWPair { r: lr.r(), w: lr.w() }),
            _ => Err(UnlockedError(())),
        }
    }
    pub fn fetch_unsafe<'a>(&'a self) -> RWPair<&'a T, &'a mut T> {
        self.fetch().expect("Called fetch on an unlock!")
    }
    #[must_use]
    pub fn commit(self) -> Result<Self, UnlockedError<Self>> {
        match self {
            Editor::Locked(lr) => Ok(Editor::Free(lr.release())),
            _ => Err(UnlockedError(self)),
        }
    }
}
#[derive(Debug)]
pub struct LockedEditor<T:Clone> {
    origin: Arc<TripleBuffer<T>>,
    rw_lock: RWPair<*const T, *mut T>,
}
impl<T:Clone> LockedEditor<T> {
    fn lock(tb: Arc<TripleBuffer<T>>) -> LockedEditor<T> {
        LockedEditor {
            rw_lock: RWPair {
                r: tb.er(),
                w: tb.ew(),
            },
            origin: tb,
        }
    }
    pub fn r<'a>(&'a self) -> &'a T {
        unsafe { &*self.rw_lock.r }
    }
    pub fn w<'a>(&'a self) -> &'a mut T {
        unsafe { &mut *self.rw_lock.w }
    }
    fn release(self) -> TB<T> {
        self.origin.advance();
        TB(self.origin)
    }
}
unsafe impl<T: Clone + Send> Send for LockedEditor<T> {}
unsafe impl<T: Clone + Sync> Sync for LockedEditor<T> {}

