use buddy_system_allocator::LockedHeap;
use core::alloc::Layout;
use core::ptr::NonNull;

const USER_HEAP_SIZE: usize = 0x70000; // 448 KB

static mut HEAP_SPACE: [u8; USER_HEAP_SIZE] = [0; USER_HEAP_SIZE];

#[global_allocator]
static HEAP: LockedHeap<32> = LockedHeap::<32>::empty();

pub struct HeapFrameTracker {
    pub ptr: NonNull<u8>,
    pub len: usize,
    align: usize,
}

unsafe impl Send for HeapFrameTracker {}

impl Drop for HeapFrameTracker {
    fn drop(&mut self) {
        HEAP.lock()
            .dealloc(self.ptr, Layout::from_size_align(self.len, self.align).unwrap());
    }
}

pub fn init() {
    unsafe {
        #[allow(static_mut_refs)]
        HEAP.lock()
            .init(HEAP_SPACE.as_ptr() as usize, USER_HEAP_SIZE);
    }
}

pub fn alloc_frames<const ALIGN: usize>(len: usize) -> HeapFrameTracker {
    let ptr = HEAP
        .lock()
        .alloc(Layout::from_size_align(len, ALIGN).unwrap())
        .unwrap();
    HeapFrameTracker { ptr, len, align: ALIGN }
}
