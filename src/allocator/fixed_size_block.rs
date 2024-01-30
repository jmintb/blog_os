use core::{
    alloc::{GlobalAlloc, Layout},
    mem,
    ptr::{self, NonNull},
};

use crate::println;

use super::Locked;

struct ListNode {
    next: Option<&'static mut ListNode>,
}

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

pub struct FixedSizeAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fall_back_allocator: linked_list_allocator::Heap,
}

impl FixedSizeAllocator {
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeAllocator {
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fall_back_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fall_back_allocator
            .init(heap_start as *mut u8, heap_size);
    }

    fn fallback_alloc(&mut self, layout: core::alloc::Layout) -> *mut u8 {
        match self.fall_back_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}

fn block_size_index(layout: Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES
        .iter()
        .position(|&block_size| block_size >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();

        if let Some(index) = block_size_index(layout) {
            if let Some(region) = allocator.list_heads[index].take() {
                allocator.list_heads[index] = region.next.take();
                region as *mut ListNode as *mut u8
            } else {
                let block_size = BLOCK_SIZES[index];
                let block_align = block_size;
                let layout = Layout::from_size_align(block_size, block_align).unwrap();
                allocator.fallback_alloc(layout)
            }
        } else {
            allocator.fallback_alloc(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();

        if let Some(index) = block_size_index(layout) {
            let new_node = ListNode {
                next: allocator.list_heads[index].take(),
            };

            assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
            assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);

            let new_node_ptr = ptr as *mut ListNode;
            new_node_ptr.write(new_node);
            allocator.list_heads[index] = Some(&mut *new_node_ptr)
        } else {
            let ptr = NonNull::new(ptr).unwrap();
            allocator.fall_back_allocator.deallocate(ptr, layout);
        }
    }
}
