//! Cache: manages a collection of slabs for a single object size.
//!
//! A cache holds slabs that all serve the same slot size. When the current
//! slab is full, the cache requests a new page and creates a new slab.
//!
//! Slabs are stored in a simple fixed-size array (no heap allocation needed).

use crate::slab::Slab;

/// Maximum number of slabs per cache.
///
/// This limits how much memory a single size class can use.
/// In a real kernel allocator this would be dynamic, but we keep it
/// static to avoid needing a heap to manage the heap.
const MAX_SLABS_PER_CACHE: usize = 32;

/// A cache manages multiple [`Slab`]s of the same object size.
pub struct Cache {
    /// The object size this cache serves.
    object_size: usize,
    /// Fixed array of slabs. `None` means the slot is unused.
    slabs: [Option<Slab>; MAX_SLABS_PER_CACHE],
    /// Number of active slabs.
    slab_count: usize,
}

impl Cache {
    /// Create a new, empty cache for objects of `object_size` bytes.
    pub const fn new(object_size: usize) -> Self {
        // We can't use array::map in const context, so we use this workaround.
        const NONE_SLAB: Option<Slab> = None;
        Cache {
            object_size,
            slabs: [NONE_SLAB; MAX_SLABS_PER_CACHE],
            slab_count: 0,
        }
    }

    /// Returns the object size this cache handles.
    pub fn object_size(&self) -> usize {
        self.object_size
    }

    /// Try to allocate an object from an existing slab.
    ///
    /// Iterates through slabs and returns from the first one that has space.
    /// Returns null if all slabs are full.
    pub fn alloc(&mut self) -> *mut u8 {
        for slot in self.slabs.iter_mut() {
            if let Some(slab) = slot {
                if !slab.is_full() {
                    return slab.alloc();
                }
            }
        }
        core::ptr::null_mut()
    }

    /// Add a new slab backed by the given page and immediately allocate from it.
    ///
    /// # Safety
    ///
    /// - `page` must be a valid, writable pointer to [`PAGE_SIZE`](crate::page::PAGE_SIZE) bytes.
    /// - The page must not be used anywhere else.
    pub unsafe fn add_slab_and_alloc(&mut self, page: *mut u8) -> *mut u8 {
        if self.slab_count >= MAX_SLABS_PER_CACHE {
            return core::ptr::null_mut();
        }

        // Safety: caller guarantees the page is valid.
        let mut slab = unsafe { Slab::new(page, self.object_size) };
        let ptr = slab.alloc();

        // Find the first empty slot to store the slab.
        for slot in self.slabs.iter_mut() {
            if slot.is_none() {
                *slot = Some(slab);
                self.slab_count += 1;
                return ptr;
            }
        }

        core::ptr::null_mut()
    }

    /// Deallocate an object.
    ///
    /// Finds the slab that contains `ptr` and frees the slot.
    ///
    /// # Safety
    ///
    /// - `ptr` must have been returned by a prior call to [`alloc`](Cache::alloc)
    ///   or [`add_slab_and_alloc`](Cache::add_slab_and_alloc) on this cache.
    /// - `ptr` must not have been deallocated already.
    pub unsafe fn dealloc(&mut self, ptr: *mut u8) -> bool {
        for slot in self.slabs.iter_mut() {
            if let Some(slab) = slot {
                if slab.contains(ptr) {
                    // Safety: ptr belongs to this slab (verified by contains()).
                    unsafe { slab.dealloc(ptr) };
                    return true;
                }
            }
        }
        false
    }
}
