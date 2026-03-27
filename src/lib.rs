//! # Rust Slab Allocator
//!
//! A minimal slab allocator for educational purposes, implemented in `no_std` Rust.
//!
//! ## Architecture
//!
//! The allocator is structured in three layers:
//!
//! - **[`slab::Slab`]**: A single page (4096 bytes) divided into fixed-size slots.
//!   Each slab maintains a free list to track available slots.
//!
//! - **[`cache::Cache`]**: Manages multiple slabs of the same object size.
//!   When a slab is full, the cache allocates a new one.
//!
//! - **[`allocator::SlabAllocator`]**: Top-level allocator that holds caches
//!   for different size classes and implements the [`GlobalAlloc`] trait.
//!
//! ## Usage
//!
//! ```ignore
//! #[global_allocator]
//! static ALLOCATOR: SlabAllocator = SlabAllocator::new();
//! ```

#![no_std]
#![warn(missing_docs)]
#![warn(unsafe_op_in_unsafe_fn)]

pub mod allocator;
pub mod cache;
pub mod page;
pub mod slab;
