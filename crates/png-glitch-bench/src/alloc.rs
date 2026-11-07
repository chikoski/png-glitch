//! A counting global allocator, so the report can show bytes allocated per iteration and the
//! live heap. It lives in the bench binary only: the library crates forbid `unsafe`.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub struct CountingAlloc;

static ALLOCATED: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicU64 = AtomicU64::new(0);

// SAFETY: every method forwards to `System` unchanged and only updates counters.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            ALLOCATED.fetch_add(layout.size() as u64, Relaxed);
            LIVE.fetch_add(layout.size() as u64, Relaxed);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            ALLOCATED.fetch_add(layout.size() as u64, Relaxed);
            LIVE.fetch_add(layout.size() as u64, Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        LIVE.fetch_sub(layout.size() as u64, Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size > layout.size() {
                let grown = (new_size - layout.size()) as u64;
                ALLOCATED.fetch_add(grown, Relaxed);
                LIVE.fetch_add(grown, Relaxed);
            } else {
                LIVE.fetch_sub((layout.size() - new_size) as u64, Relaxed);
            }
        }
        new_ptr
    }
}

/// Total bytes requested from the allocator since start-up (never decreases).
pub fn allocated_bytes() -> u64 {
    ALLOCATED.load(Relaxed)
}

/// Bytes currently allocated and not yet freed.
pub fn live_bytes() -> u64 {
    LIVE.load(Relaxed)
}
