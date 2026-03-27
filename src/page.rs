//! Page provider abstraction.
//!
//! In a real kernel, pages come from the physical frame allocator.
//! This module defines the [`PageProvider`] trait so that the slab allocator
//! stays decoupled from the actual page source.

/// Standard page size in bytes (4 KiB).
pub const PAGE_SIZE: usize = 4096;

/// Trait for providing raw pages to the slab allocator.
///
/// # Safety
///
/// Implementors must guarantee that:
/// - [`alloc_page`](PageProvider::alloc_page) returns a pointer to a
///   `PAGE_SIZE`-aligned region of at least `PAGE_SIZE` bytes, or null on failure.
/// - [`dealloc_page`](PageProvider::dealloc_page) is only called with pointers
///   previously returned by [`alloc_page`](PageProvider::alloc_page).
/// - A page is not deallocated more than once.
pub unsafe trait PageProvider {
    /// Allocate a single page.
    ///
    /// Returns a pointer to the start of a `PAGE_SIZE`-byte region,
    /// or a null pointer if allocation fails.
    fn alloc_page(&self) -> *mut u8;

    /// Deallocate a page previously allocated by [`alloc_page`](PageProvider::alloc_page).
    ///
    /// # Safety
    ///
    /// - `ptr` must have been returned by a prior call to `alloc_page`.
    /// - `ptr` must not have been deallocated already.
    unsafe fn dealloc_page(&self, ptr: *mut u8);
}

/// A simple page provider backed by a static byte array.
///
/// This is useful for testing and for environments where no
/// dynamic page allocator exists yet. Pages are handed out
/// sequentially and never truly freed (bump allocator style).
pub struct StaticPageProvider<const N: usize> {
    /// The backing storage. Must be large enough to hold multiple pages.
    heap: *mut u8,
    /// Offset tracking the next free position.
    offset: spin::Mutex<usize>,
    /// Total capacity in bytes.
    capacity: usize,
}

impl<const N: usize> StaticPageProvider<N> {
    /// Create a new static page provider from a mutable byte slice.
    ///
    /// # Safety
    ///
    /// - `heap_space` must live for the entire duration of allocator usage.
    /// - `heap_space` must not be aliased or accessed elsewhere while the
    ///   provider is in use.
    pub unsafe fn new(heap_space: &mut [u8; N]) -> Self {
        let ptr = heap_space.as_mut_ptr();
        // Align up to PAGE_SIZE
        let align_offset = ptr.align_offset(PAGE_SIZE);
        Self {
            heap: unsafe { ptr.add(align_offset) },
            offset: spin::Mutex::new(0),
            capacity: N.saturating_sub(align_offset),
        }
    }
}

// Safety: StaticPageProvider returns properly aligned, non-overlapping pages
// from its backing array. Deallocation is a no-op (bump allocator).
unsafe impl<const N: usize> PageProvider for StaticPageProvider<N> {
    fn alloc_page(&self) -> *mut u8 {
        let mut offset = self.offset.lock();
        if *offset + PAGE_SIZE > self.capacity {
            return core::ptr::null_mut();
        }

        // Safety: offset is within bounds and heap is valid for capacity bytes.
        let page = unsafe { self.heap.add(*offset) };
        *offset += PAGE_SIZE;
        page
    }

    unsafe fn dealloc_page(&self, _ptr: *mut u8) {
        // Bump allocator: pages are not individually freed.
        // A real implementation would return the page to a free list.
    }
}

// Safety: The inner state is protected by a spinlock.
unsafe impl<const N: usize> Send for StaticPageProvider<N> {}
unsafe impl<const N: usize> Sync for StaticPageProvider<N> {}
