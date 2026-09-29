//! First-fit heap. Blocks are contiguous headers followed by a payload.
//! Alignment above 16 bytes is rejected; the kernel only allocates smaller types.

use core::alloc::Layout;

const ALIGN: usize = 16;
const MAGIC: u64 = 0xA110_CA7E_u64;

#[repr(C)]
struct Header {
    magic: u64,
    size: usize,
    free: bool,
}

pub struct Heap {
    start: usize,
    end: usize,
    used: usize,
}

impl Heap {
    pub const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            used: 0,
        }
    }

    pub fn total(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn used(&self) -> usize {
        self.used
    }

    /// # Safety
    /// `start` must point at `size` writable bytes that outlive the heap.
    pub unsafe fn init(&mut self, start: usize, size: usize) {
        let raw_end = start.saturating_add(size);
        let start = align_up(start, ALIGN);
        let end = raw_end & !(ALIGN - 1);
        if start >= end || end - start < header_size() + ALIGN {
            self.start = 0;
            self.end = 0;
            self.used = 0;
            return;
        }
        unsafe {
            (start as *mut Header).write(Header {
                magic: MAGIC,
                size: end - start,
                free: true,
            });
        }
        self.start = start;
        self.end = end;
        self.used = 0;
    }

    pub fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if self.start == 0 || layout.align() > ALIGN {
            return core::ptr::null_mut();
        }
        let need = align_up(layout.size().max(1), ALIGN);
        let mut cursor = self.start;
        while cursor + header_size() <= self.end {
            let header = cursor as *mut Header;
            let header_ref = unsafe { &*header };
            if header_ref.magic != MAGIC || header_ref.size < header_size() {
                return core::ptr::null_mut();
            }
            let block = header_ref.size;
            if header_ref.free && block >= header_size() + need {
                let leftover = block - header_size() - need;
                if leftover >= header_size() + ALIGN {
                    unsafe {
                        (*header).size = header_size() + need;
                        (*header).free = false;
                        let next = (cursor + (*header).size) as *mut Header;
                        next.write(Header {
                            magic: MAGIC,
                            size: leftover,
                            free: true,
                        });
                    }
                } else {
                    unsafe { (*header).free = false };
                }
                self.used = self.used.saturating_add(need);
                return unsafe { (cursor as *mut u8).add(header_size()) };
            }
            cursor += block;
        }
        core::ptr::null_mut()
    }

    /// # Safety
    /// `ptr` must have come from [`Heap::alloc`] on this heap.
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() || self.start == 0 {
            return;
        }
        let header = ptr.sub(header_size()) as *mut Header;
        if header as usize + header_size() > self.end || unsafe { (*header).magic } != MAGIC {
            return;
        }
        unsafe { (*header).free = true };
        let need = align_up(layout.size().max(1), ALIGN);
        self.used = self.used.saturating_sub(need);
        self.coalesce();
    }

    fn coalesce(&mut self) {
        let mut cursor = self.start;
        while cursor + header_size() <= self.end {
            let header = cursor as *mut Header;
            let size = unsafe { (*header).size };
            if size < header_size() || cursor + size > self.end {
                return;
            }
            let next = cursor + size;
            if next + header_size() <= self.end {
                let next_header = next as *mut Header;
                let next_size = unsafe { (*next_header).size };
                if unsafe { (*header).free && (*next_header).free } && next_size >= header_size()
                {
                    unsafe { (*header).size = size + next_size };
                    continue;
                }
            }
            cursor = next;
        }
    }
}

fn header_size() -> usize {
    align_up(core::mem::size_of::<Header>(), ALIGN)
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocate_write_and_free() {
        let mut storage = vec![0u8; 64 * 1024];
        let mut heap = Heap::empty();
        unsafe { heap.init(storage.as_mut_ptr() as usize, storage.len()) };
        assert!(heap.total() > 1024);
        let layout = Layout::from_size_align(24, 8).unwrap();
        let first = heap.alloc(layout);
        let second = heap.alloc(layout);
        assert!(!first.is_null() && !second.is_null());
        assert_ne!(first, second);
        unsafe {
            first.write_bytes(0xAB, 24);
            assert_eq!(*first, 0xAB);
            heap.dealloc(first, layout);
        }
        let third = heap.alloc(layout);
        assert_eq!(third, first);
        unsafe { heap.dealloc(second, layout) };
        unsafe { heap.dealloc(third, layout) };
        assert_eq!(heap.used(), 0);
    }

    #[test]
    fn rejects_over_aligned_layout() {
        let mut storage = vec![0u8; 4096];
        let mut heap = Heap::empty();
        unsafe { heap.init(storage.as_mut_ptr() as usize, storage.len()) };
        let layout = Layout::from_size_align(8, 32).unwrap();
        assert!(heap.alloc(layout).is_null());
    }
}
