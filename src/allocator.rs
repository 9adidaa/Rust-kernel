//! Top-level slab allocator implementing [`GlobalAlloc`].
//!
//! The allocator maintains caches for common size classes (powers of two).
//! Allocation requests are rounded up to the nearest size class and
//! dispatched to the appropriate cache.
//!
//! ## Size Classes
//!
//! | Class | Slot Size | Slots per Page |
//! |-------|-----------|----------------|
//! | 0     | 8 B       | 512            |
//! | 1     | 16 B      | 256            |
//! | 2     | 32 B      | 128            |
//! | 3     | 64 B      | 64             |
//! | 4     | 128 B     | 32             |
//! | 5     | 256 B     | 16             |
//! | 6     | 512 B     | 8              |
//! | 7     | 1024 B    | 4              |
//! | 8     | 2048 B    | 2              |

use crate::cache::Cache;
use crate::page::PageProvider;
use core::alloc::{GlobalAlloc, Layout};
use spin::Mutex;

/// Number of size classes: 8, 16, 32, 64, 128, 256, 512, 1024, 2048.
const NUM_SIZE_CLASSES: usize = 9;

/// The size classes available (powers of 2 from 8 to 2048).
const SIZE_CLASSES: [usize; NUM_SIZE_CLASSES] = [8, 16, 32, 64, 128, 256, 512, 1024, 2048];

/// Find the index of the smallest size class that fits `size`.
///
/// Returns `None` if the requested size exceeds the largest size class.
fn size_class_index(size: usize) -> Option<usize> {
    SIZE_CLASSES.iter().position(|&s| s >= size)
}

/// The top-level slab allocator.
///
/// Wraps an array of [`Cache`]s (one per size class) behind a spinlock
/// for thread safety, and a reference to a [`PageProvider`] for
/// obtaining backing pages.
pub struct SlabAllocator<P: PageProvider> {
    inner: Mutex<SlabAllocatorInner>,
    page_provider: P,
}

struct SlabAllocatorInner {
    caches: [Cache; NUM_SIZE_CLASSES],
}

impl<P: PageProvider> SlabAllocator<P> {
    /// Create a new slab allocator backed by the given page provider.
    pub fn new(page_provider: P) -> Self {
        let caches = [
            Cache::new(SIZE_CLASSES[0]),
            Cache::new(SIZE_CLASSES[1]),
            Cache::new(SIZE_CLASSES[2]),
            Cache::new(SIZE_CLASSES[3]),
            Cache::new(SIZE_CLASSES[4]),
            Cache::new(SIZE_CLASSES[5]),
            Cache::new(SIZE_CLASSES[6]),
            Cache::new(SIZE_CLASSES[7]),
            Cache::new(SIZE_CLASSES[8]),
        ];

        SlabAllocator {
            inner: Mutex::new(SlabAllocatorInner { caches }),
            page_provider,
        }
    }
}

// Safety: The allocator's mutable state is protected by a spinlock.
// Allocation requests are served from size-class caches. When a cache
// is full, a new page is requested from the page provider and converted
// into a new slab. The GlobalAlloc contract is upheld:
// - `alloc` returns a pointer aligned to `layout.align()` (guaranteed
//   because size classes are powers of two and pages are page-aligned).
// - `dealloc` only frees pointers previously returned by `alloc`.
unsafe impl<P: PageProvider + Sync> GlobalAlloc for SlabAllocator<P> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());

        let index = match size_class_index(size) {
            Some(i) => i,
            None => return core::ptr::null_mut(), // Too large for our allocator
        };

        let mut inner = self.inner.lock();
        let cache = &mut inner.caches[index];

        // Try allocating from an existing slab first.
        let ptr = cache.alloc();
        if !ptr.is_null() {
            return ptr;
        }

        // All slabs full — request a new page and create a new slab.
        let page = self.page_provider.alloc_page();
        if page.is_null() {
            return core::ptr::null_mut();
        }

        // Safety: page_provider returned a valid page.
        unsafe { cache.add_slab_and_alloc(page) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align());

        let index = match size_class_index(size) {
            Some(i) => i,
            None => return, // Should not happen if ptr came from us
        };

        let mut inner = self.inner.lock();
        let cache = &mut inner.caches[index];

        // Safety: caller guarantees ptr was returned by our alloc().
        unsafe {
            cache.dealloc(ptr);
        }
    }
}
