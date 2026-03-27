//! Slab: a single page divided into fixed-size object slots.
//!
//! A slab occupies exactly one page ([`PAGE_SIZE`](crate::page::PAGE_SIZE) bytes).
//! It is divided into `N` equally-sized slots, where each slot can hold one
//! object. Free slots are linked together using an embedded free list:
//! the first bytes of each free slot store a pointer to the next free slot.
//!
//! ```text
//! ┌──────────┬──────────┬──────────┬───┬──────────┐
//! │  slot 0  │  slot 1  │  slot 2  │...│  slot N  │
//! │ [*next]  │ [object] │ [*next]  │   │ [null]   │
//! └──────────┴──────────┴──────────┴───┴──────────┘
//!     free       used       free              free
//! ```

use crate::page::PAGE_SIZE;
use core::ptr;

/// Minimum object size: must be large enough to store a pointer for the free list.
const MIN_SLOT_SIZE: usize = core::mem::size_of::<*mut u8>();

/// A slab manages a single page of fixed-size slots.
pub struct Slab {
    /// Pointer to the start of the page backing this slab.
    page_ptr: *mut u8,
    /// Size of each slot in bytes.
    slot_size: usize,
    /// Number of total slots in this slab.
    total_slots: usize,
    /// Number of currently allocated (used) slots.
    used_slots: usize,
    /// Pointer to the first free slot (head of the embedded free list).
    free_head: *mut u8,
}

impl Slab {
    /// Initialize a new slab on top of an already-allocated page.
    ///
    /// This divides the page into slots of `object_size` bytes and builds
    /// the embedded free list linking all slots together.
    ///
    /// # Safety
    ///
    /// - `page` must point to a valid, writable region of at least
    ///   [`PAGE_SIZE`] bytes.
    /// - `page` must be properly aligned (at least to `object_size`).
    /// - The caller must ensure no other code accesses this page while
    ///   the slab is in use.
    pub unsafe fn new(page: *mut u8, object_size: usize) -> Self {
        let slot_size = object_size.max(MIN_SLOT_SIZE);
        let total_slots = PAGE_SIZE / slot_size;

        assert!(total_slots > 0, "object_size too large for a single page");

        // Build the free list: each free slot points to the next one.
        // The last slot points to null.
        for i in 0..total_slots {
            let slot = unsafe { page.add(i * slot_size) };
            let next = if i + 1 < total_slots {
                unsafe { page.add((i + 1) * slot_size) }
            } else {
                ptr::null_mut()
            };
            // Safety: slot is within the page and is large enough to hold a pointer.
            unsafe {
                (slot as *mut *mut u8).write(next);
            }
        }

        Slab {
            page_ptr: page,
            slot_size,
            total_slots,
            used_slots: 0,
            free_head: page,
        }
    }

    /// Allocate one object slot from this slab.
    ///
    /// Returns a pointer to the allocated slot, or null if the slab is full.
    pub fn alloc(&mut self) -> *mut u8 {
        if self.free_head.is_null() {
            return ptr::null_mut();
        }

        // Pop the head of the free list.
        let slot = self.free_head;

        // Safety: free_head is a valid slot within our page, and it contains
        // a pointer to the next free slot (or null).
        self.free_head = unsafe { (slot as *mut *mut u8).read() };
        self.used_slots += 1;

        slot
    }

    /// Deallocate an object slot, returning it to the slab's free list.
    ///
    /// # Safety
    ///
    /// - `ptr` must have been returned by a prior call to [`alloc`](Slab::alloc)
    ///   on this same slab.
    /// - `ptr` must not have been deallocated already (no double free).
    pub unsafe fn dealloc(&mut self, ptr: *mut u8) {
        // Push the freed slot onto the head of the free list.
        // Safety: ptr is a valid slot within our page. We write the current
        // free_head into it, making it the new head.
        unsafe {
            (ptr as *mut *mut u8).write(self.free_head);
        }
        self.free_head = ptr;
        self.used_slots -= 1;
    }

    /// Returns `true` if all slots are allocated.
    pub fn is_full(&self) -> bool {
        self.used_slots == self.total_slots
    }

    /// Returns `true` if no slots are allocated.
    pub fn is_empty(&self) -> bool {
        self.used_slots == 0
    }

    /// Returns the total number of slots in this slab.
    pub fn total_slots(&self) -> usize {
        self.total_slots
    }

    /// Returns the number of currently used slots.
    pub fn used_slots(&self) -> usize {
        self.used_slots
    }

    /// Returns the pointer to the page backing this slab.
    pub fn page_ptr(&self) -> *mut u8 {
        self.page_ptr
    }

    /// Returns `true` if `ptr` falls within this slab's page.
    pub fn contains(&self, ptr: *mut u8) -> bool {
        let start = self.page_ptr as usize;
        let end = start + PAGE_SIZE;
        let addr = ptr as usize;
        addr >= start && addr < end
    }
}
