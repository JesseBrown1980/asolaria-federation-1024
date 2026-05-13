#![no_std]
#![no_main]

mod init;

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicUsize, Ordering};

struct BumpAllocator {
    heap_start: AtomicUsize,
    heap_end: AtomicUsize,
    next: AtomicUsize,
}

impl BumpAllocator {
    const fn new() -> Self {
        BumpAllocator {
            heap_start: AtomicUsize::new(0),
            heap_end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
        }
    }
    unsafe fn init(&self, start: usize, size: usize) {
        self.heap_start.store(start, Ordering::Relaxed);
        self.heap_end.store(start + size, Ordering::Relaxed);
        self.next.store(start, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align();
        let size = layout.size();
        let heap_end = self.heap_end.load(Ordering::Relaxed);
        let mut current = self.next.load(Ordering::Relaxed);
        loop {
            let aligned = (current + align - 1) & !(align - 1);
            let new_next = aligned + size;
            if new_next > heap_end {
                return core::ptr::null_mut();
            }
            match self.next.compare_exchange_weak(current, new_next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => return aligned as *mut u8,
                Err(actual) => current = actual,
            }
        }
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

const HEAP_SIZE: usize = 16 * 1024;

static mut HEAP: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("hlt", options(nomem, nostack)); } }
}

#[no_mangle]
pub extern "C" fn main() -> ! {
    let _anchor = asolaria_kernel_core::FEDERATION_ANCHOR_PID;
    unsafe {
        let heap_ptr = core::ptr::addr_of_mut!(HEAP);
        ALLOCATOR.init(heap_ptr as *mut u8 as usize, HEAP_SIZE);
    }
    // Phase-2 Step 31 — boot to envelope-shell via minimal init system.
    // init::run() diverges; only returns by sys_exit. If init somehow returns
    // (shouldn't be possible per its signature), fall through to hlt-loop as
    // a defense-in-depth safeguard.
    init::run();
}
