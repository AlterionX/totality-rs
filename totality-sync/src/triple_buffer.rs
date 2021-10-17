use std::{
    cell::UnsafeCell,
    sync::Arc,
    marker::Send,
};

use cb::utils::CachePadded;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

#[cfg(feature = "sub")]
mod tb {
    use std::sync::atomic::{AtomicU8, Ordering};
    use cb::utils::CachePadded;
    #[allow(unused_imports)]
    use log::{debug, error, info, trace, warn};

    #[derive(Debug)]
    pub struct TripleBufferIndices {
        pub snatched_read: CachePadded<u8>,     // unique
        packed: CachePadded<AtomicU8>, // shared
        pub edit_rw: CachePadded<(u8, u8)>,  // unique
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
        pub fn snatch(&mut self) {
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
        pub fn advance(&mut self) {
            let curr_read = self.edit_rw.1;
            let curr_write = Self::unpack(self.packed.swap(
                Self::pack(true, curr_read),
                std::sync::atomic::Ordering::Release,
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
}

#[cfg(any(feature = "fna_usize", feature = "fna"))]
mod tb {
    #[cfg(feature = "fna")]
    use std::sync::atomic::{AtomicU8, Ordering};
    #[cfg(feature = "fna_usize")]
    use std::sync::atomic::{AtomicUsize, Ordering};
    use cb::utils::CachePadded;
    #[allow(unused_imports)]
    use log::{debug, error, info, trace, warn};

    #[cfg(feature = "fna")]
    type SyncType = AtomicU8;
    #[cfg(feature = "fna_usize")]
    type SyncType = AtomicUsize;
    #[cfg(feature = "fna")]
    type MaskType = u8;
    #[cfg(feature = "fna_usize")]
    type MaskType = usize;
    #[cfg(feature = "fna")]
    type Index = u8;
    #[cfg(feature = "fna_usize")]
    type Index = usize;

    #[derive(Debug)]
    pub struct TripleBufferIndices {
        pub snatched_read: CachePadded<Index>,     // unique
        packed: CachePadded<SyncType>, // shared
        pub edit_rw: CachePadded<(Index, Index)>,  // unique
    }
    impl TripleBufferIndices {
        const BUFFER_ID_MASK: MaskType = 0b11;
        const IS_NEW_MASK: MaskType = 0b100;
        #[inline]
        fn pack(v: MaskType) -> MaskType {
            !v & Self::BUFFER_ID_MASK
        }
        #[inline]
        fn unpack(packed: MaskType) -> (bool, Index) {
            let is_new = (packed & Self::IS_NEW_MASK) == 0;
            let next_write = !packed & Self::BUFFER_ID_MASK;
            (is_new, next_write)
        }
        #[inline]
        fn mask(v: MaskType) -> MaskType {
            match v {
                0 => 0b000,
                1 => 0b001,
                2 => 0b010,
                _ => panic!("We done goofed!"),
            }
        }
    }
    impl TripleBufferIndices {
        pub fn snatch(&mut self) {
            if Self::unpack(self.packed.load(Ordering::Acquire)).0 {
                let old_snatched = self.snatched_read;
                *self.snatched_read = Self::unpack(
                    self.packed.fetch_nand(Self::mask(*old_snatched), Ordering::AcqRel),
                )
                .1;
                trace!(
                    "Snatching indices {:?} and returning indices {:?}.",
                    old_snatched,
                    self.snatched_read
                );
            }
        }
        pub fn advance(&mut self) {
            let curr_read = self.edit_rw.1;
            let curr_write = Self::unpack(self.packed.swap(
                Self::pack(curr_read),
                Ordering::Release,
            ))
            .1;
            trace!(
                "Advancing indices from {:?} to {:?}.",
                curr_read,
                curr_write
            );
            *self.edit_rw = (curr_read, curr_write);
        }
    }
    impl Default for TripleBufferIndices {
        fn default() -> Self {
            Self {
                snatched_read: CachePadded::new(0),
                packed: CachePadded::new(SyncType::new(Self::pack(1))),
                edit_rw: CachePadded::new((1, 2)),
            }
        }
    }
}

#[cfg(feature = "old")]
mod tb {
    use std::sync::atomic::{AtomicU8, AtomicUsize, AtomicBool, Ordering};
    use cb::utils::CachePadded;
    #[allow(unused_imports)]
    use log::{debug, error, info, trace, warn};

    #[derive(Debug)]
    pub struct TripleBufferIndices {
        pub snatched_read: CachePadded<usize>,     // unique
        packed_vals: CachePadded<AtomicUsize>, // shared
        stale: CachePadded<AtomicBool>,        // shared
        pub edit_rw: CachePadded<(usize, usize)>,  // unique
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
        pub fn snatch(&mut self) {
            let mask = (0b1 << 4)
                + (0b11 << 2)
                + match *self.snatched_read {
                    0 => 0b00,
                    1 => 0b01,
                    2 => 0b10,
                    _ => panic!("We done goofed!"),
                };
            if !self.stale.swap(true, std::sync::atomic::Ordering::Acquire) {
                let old_snatched = *self.snatched_read;
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
        pub fn advance(&mut self) {
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
            self.edit_rw = (self.edit_rw.1, next_write);
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
}

// NOTE Dummy doesn't actually work, don't use it outside of benchmarking
#[cfg(all(feature = "dummy", test))]
mod tb {
    use std::{
        sync::{
            atomic::{AtomicUsize, AtomicBool},
        },
    };
    use cb::utils::CachePadded;

    #[derive(Debug)]
    pub struct TripleBufferIndices {
        pub snatched_read: CachePadded<usize>,     // unique
        packed_vals: CachePadded<AtomicUsize>, // shared
        stale: CachePadded<AtomicBool>,        // shared
        pub edit_rw: CachePadded<(usize, usize)>,  // unique
    }
    impl TripleBufferIndices {
        pub fn snatch(&mut self) {
        }
        pub fn advance(&mut self) {
        }
    }
    impl Default for TripleBufferIndices {
        fn default() -> Self {
            Self {
                snatched_read: CachePadded::new(0),
                packed_vals: CachePadded::new(AtomicUsize::new(0)),
                stale: CachePadded::new(AtomicBool::new(true)),
                edit_rw: CachePadded::new((1, 2)),
            }
        }
    }
}

use tb::TripleBufferIndices;

#[derive(Debug, Clone)]
pub struct TB<T:Clone>(Arc<TripleBuffer<T>>);

#[derive(Debug)]
pub struct TripleBuffer<T: Clone> {
    ii: UnsafeCell<TripleBufferIndices>,
    backing_mem: *const [UnsafeCell<CachePadded<T>>; 3],
    tt: [*mut T; 3],
}
impl<T: Clone> TripleBuffer<T> {
    #[inline]
    fn alloc(src: T) -> TB<T> {
        TB(Self::raw(src))
    }
    pub fn raw(src: T) -> Arc<TripleBuffer<T>> {
        use std::mem::MaybeUninit;
        let backing_mem = Box::into_raw(Box::new([
            UnsafeCell::new(CachePadded::new(src.clone())),
            UnsafeCell::new(CachePadded::new(src.clone())),
            UnsafeCell::new(CachePadded::new(src)),
        ]));
        let mut tt: MaybeUninit<[*mut T; 3]> = MaybeUninit::uninit();
        unsafe {
            for i in 0..3 {
                (*tt.as_mut_ptr())[i] = &mut **(*backing_mem)[i].get();
            }
        }
        Arc::new(Self {
            ii: UnsafeCell::new(TripleBufferIndices::default()),
            backing_mem,
            tt: unsafe { tt.assume_init() },
        })
    }
    #[inline]
    pub fn snatch(&self) {
        let ii = self.ii.get();
        unsafe { (*ii).snatch() };
    }
    #[inline]
    pub fn advance(&self) {
        let ii = self.ii.get();
        unsafe { (*ii).advance() };
    }
    #[inline]
    fn rr(&self) -> *const T {
        let ii = self.ii.get();
        self.tt[unsafe { *(*ii).snatched_read as usize }]
    }
    #[inline]
    fn er(&self) -> *const T {
        let ii = self.ii.get();
        self.tt[unsafe { (*ii).edit_rw.0 as usize }]
    }
    #[inline]
    fn ew(&self) -> *mut T {
        let ii = self.ii.get();
        self.tt[unsafe { (*ii).edit_rw.1 as usize }]
    }
    #[inline]
    pub fn reader_r(&self) -> & T {
        unsafe { & *self.rr() }
    }
    #[inline]
    pub fn editor_r(&self) -> & T {
        unsafe { & *self.er() }
    }
    #[inline]
    pub fn editor_w(&self) -> &mut T {
        unsafe { &mut *self.ew() }
    }
}
impl<T: Clone> Drop for TripleBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            Box::from_raw(self.backing_mem as *mut [CachePadded<T>; 3]);
        };
    }
}
unsafe impl<T: Clone + Send> Send for TripleBuffer<T> {}
unsafe impl<T: Clone + Sync> Sync for TripleBuffer<T> {}

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
            Reader::Free(tb) => Ok(Reader::Locked(LockedReader::lock(tb.0))),
            _ => Err(LockedError(self)), // TODO consider returning self
        }
    }
    pub fn grab_always(self) -> Self {
        match self {
            Reader::Free(tb) => Reader::Locked(LockedReader::lock(tb.0)),
            _ => self,
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
        match self {
            Reader::Locked(lr) => lr.r(),
            _ => panic!("Attempted to get buffers from old buffer."),
        }
    }
    #[must_use]
    pub fn release(self) -> Result<Self, UnlockedError<Self>> {
        match self {
            Reader::Locked(lr) => Ok(Reader::Free(lr.release())),
            _ => Err(UnlockedError(self)), // TODO consider returning self
        }
    }
    pub fn release_always(self) -> Self {
        match self {
            Reader::Locked(lr) => Reader::Free(lr.release()),
            _ => self,
        }
    }
}
#[derive(Debug)]
pub struct LockedReader<T:Clone> {
    origin: Arc<TripleBuffer<T>>,
    locked: *const T,
}
impl<T:Clone> LockedReader<T> {
    fn lock(rv: Arc<TripleBuffer<T>>) -> LockedReader<T> {
        rv.snatch();
        Self {
            locked: rv.rr(),
            origin: rv,
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
    pub fn grab_always(self) -> Self {
        match self {
            Editor::Free(tb) => Editor::Locked(LockedEditor::lock(tb.0)),
            _ => self,
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
    pub fn commit_always(self) -> Self {
        match self {
            Editor::Locked(lr) => Editor::Free(lr.release()),
            _ => self,
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

pub fn buffer<T:Clone>(src: T) -> (Reader<T>, Editor<T>) {
    let arc = TripleBuffer::alloc(src);
    (Reader::Free(arc.clone()), Editor::Free(arc))
}

