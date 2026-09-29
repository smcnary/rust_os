//! Install the global allocator over a usable Limine memory-map region.

use crate::boot;
use crate::heap::Heap;
use core::alloc::{GlobalAlloc, Layout};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Mutex;

struct LockedHeap(Mutex<Heap>);

unsafe impl GlobalAlloc for LockedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.lock().alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.lock().dealloc(ptr, layout);
    }
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap(Mutex::new(Heap::empty()));

static USABLE: AtomicU64 = AtomicU64::new(0);

const FALLBACK_LEN: usize = 128 * 1024;
static mut FALLBACK: [u8; FALLBACK_LEN] = [0; FALLBACK_LEN];

pub struct Stats {
    pub heap_bytes: usize,
    pub used_bytes: usize,
    pub usable_ram: u64,
}

pub fn init() {
    let mut usable = 0u64;
    let mut chosen: Option<(u64, u64)> = None;
    for (base, length) in boot::usable_regions() {
        usable = usable.saturating_add(length);
        let better = match chosen {
            Some((_, best)) => length > best,
            None => true,
        };
        if better && length >= 64 * 1024 {
            chosen = Some((base, length));
        }
    }
    USABLE.store(usable, Ordering::Relaxed);

    if let (Some((base, length)), Some(offset)) = (chosen, boot::hhdm_offset()) {
        let capped = length.min(4 * 1024 * 1024);
        let virt = base.saturating_add(offset) as usize;
        unsafe { ALLOCATOR.0.lock().init(virt, capped as usize) };
        return;
    }

    let start = core::ptr::addr_of_mut!(FALLBACK) as usize;
    unsafe { ALLOCATOR.0.lock().init(start, FALLBACK_LEN) };
}

pub fn stats() -> Stats {
    let heap = ALLOCATOR.0.lock();
    Stats {
        heap_bytes: heap.total(),
        used_bytes: heap.used(),
        usable_ram: USABLE.load(Ordering::Relaxed),
    }
}
