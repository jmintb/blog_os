use core::{alloc::GlobalAlloc, ptr};

use super::Locked;

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut allocator = self.lock();
        let alloc_start = align_up(allocator.next, layout.align());

        let allocation_end = match alloc_start.checked_add(layout.size()) {
            Some(res) => res,
            _ => return ptr::null_mut(),
        };

        if allocation_end > allocator.heap_end {
            return ptr::null_mut();
        }

        allocator.next = allocation_end;
        allocator.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        let mut allocator = self.lock();

        allocator.allocations -= 1;

        if allocator.allocations == 0 {
            allocator.next = allocator.heap_start;
        }
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;
    if addr % align == 0 {
        addr - remainder + align
    } else {
        addr
    }
}
