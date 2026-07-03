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
            match self.next.compare_exchange_weak(
                current,
                new_next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
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
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
    }
}

// ---- Minimal UEFI console output (no external crate) ----
// Just enough of the EFI System Table + Simple Text Output Protocol to print a boot banner via
// firmware ConOut. Field offsets follow the UEFI spec exactly (repr(C) + the natural pad after
// `firmware_revision`); we only read up to `con_out`, so omitting later fields is sound.

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
struct SimpleTextOutput {
    reset: usize,
    output_string: unsafe extern "efiapi" fn(*mut SimpleTextOutput, *const u16) -> usize,
    // remaining function pointers omitted — never called
}

#[repr(C)]
struct EfiSystemTable {
    hdr: EfiTableHeader,
    firmware_vendor: *const u16,
    firmware_revision: u32,
    console_in_handle: *mut core::ffi::c_void,
    con_in: *mut core::ffi::c_void,
    console_out_handle: *mut core::ffi::c_void,
    con_out: *mut SimpleTextOutput,
    standard_error_handle: *mut core::ffi::c_void,
    std_err: *mut core::ffi::c_void,
    runtime_services: *mut core::ffi::c_void,
    boot_services: *mut BootServices,
    // remaining fields omitted — never read
}

// ---- UEFI Boot Services (only LocateProtocol) + Graphics Output Protocol ----
// So the kernel can OWN the screen instead of leaving OVMF's logo up: locate GOP, take the
// framebuffer, and paint "ASOLARIA ASI OS" ourselves.

#[repr(C)]
struct EfiGuid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const GOP_GUID: EfiGuid = EfiGuid {
    d1: 0x9042a9de,
    d2: 0x23dc,
    d3: 0x4a38,
    d4: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a],
};

#[repr(C)]
struct BootServices {
    hdr: EfiTableHeader,
    // 37 function pointers: RaiseTPL .. LocateHandleBuffer (UEFI 2.x order),
    // then LocateProtocol at offset 24 + 37*8 = 320.
    _pad: [usize; 37],
    locate_protocol: unsafe extern "efiapi" fn(
        *const EfiGuid,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> usize,
    // remaining fields omitted
}

#[repr(C)]
struct GopModeInfo {
    version: u32,
    horizontal_resolution: u32,
    vertical_resolution: u32,
    pixel_format: u32, // 0=RGBReserved8, 1=BGRReserved8
    pixel_information: [u32; 4],
    pixels_per_scan_line: u32,
}

#[repr(C)]
struct GopMode {
    max_mode: u32,
    mode: u32,
    info: *mut GopModeInfo,
    size_of_info: usize,
    framebuffer_base: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct GraphicsOutputProtocol {
    query_mode: usize,
    set_mode: usize,
    blt: usize,
    mode: *mut GopMode,
}

/// Pack r,g,b into the framebuffer's pixel format (BGR is the common case; format 0 is RGB).
fn pack(fmt: u32, r: u32, g: u32, b: u32) -> u32 {
    if fmt == 0 {
        (b << 16) | (g << 8) | r
    } else {
        (r << 16) | (g << 8) | b
    }
}

/// 8x8 glyphs for exactly the characters in "ASOLARIA ASI OS" (+ space). MSB = leftmost pixel.
fn glyph(c: u8) -> [u8; 8] {
    match c {
        b'A' => [0x38, 0x6C, 0xC6, 0xC6, 0xFE, 0xC6, 0xC6, 0x00],
        b'S' => [0x7C, 0xC6, 0xC0, 0x7C, 0x06, 0xC6, 0x7C, 0x00],
        b'O' => [0x7C, 0xC6, 0xC6, 0xC6, 0xC6, 0xC6, 0x7C, 0x00],
        b'L' => [0xC0, 0xC0, 0xC0, 0xC0, 0xC0, 0xC6, 0xFE, 0x00],
        b'R' => [0xFC, 0xC6, 0xC6, 0xFC, 0xD8, 0xCC, 0xC6, 0x00],
        b'I' => [0x7C, 0x18, 0x18, 0x18, 0x18, 0x18, 0x7C, 0x00],
        _ => [0; 8], // space / unknown
    }
}

/// Fill the whole framebuffer with a single color.
unsafe fn fb_fill(fb: *mut u32, pps: u32, w: u32, h: u32, color: u32) {
    for y in 0..h {
        let row = fb.add((y * pps) as usize);
        for x in 0..w {
            *row.add(x as usize) = color;
        }
    }
}

/// Draw an ASCII string at (x0,y0), each glyph scaled up by `scale`, in `color`.
unsafe fn fb_string(fb: *mut u32, pps: u32, s: &[u8], x0: u32, y0: u32, scale: u32, color: u32) {
    let mut cx = x0;
    for &c in s {
        let g = glyph(c);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..8u32 {
                if bits & (0x80 >> col) != 0 {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let px = cx + col * scale + dx;
                            let py = y0 + row as u32 * scale + dy;
                            *fb.add((py * pps + px) as usize) = color;
                        }
                    }
                }
            }
        }
        cx += 9 * scale; // 8px glyph + 1px spacing
    }
}

/// Print an ASCII line to the UEFI console (zero-extended to UTF-16, null-terminated).
/// Best-effort: silently no-ops if the table or `con_out` is null.
unsafe fn uefi_print(st: *mut EfiSystemTable, msg: &[u8]) {
    if st.is_null() || (*st).con_out.is_null() {
        return;
    }
    let con_out = (*st).con_out;
    let mut buf = [0u16; 128];
    let mut i = 0;
    while i < msg.len() && i < buf.len() - 1 {
        buf[i] = msg[i] as u16;
        i += 1;
    }
    buf[i] = 0;
    ((*con_out).output_string)(con_out, buf.as_ptr());
}

// ---- Direct COM1 serial output (16550 UART @ 0x3F8) ----
// Independent of UEFI ConOut routing — guarantees a visible boot trace on the serial console
// (QEMU `-serial`, or a real machine's COM1). ConOut drives the graphics screen; serial is the
// headless/debug lane. Together they cover both "on a monitor" and "over a cable".
const COM1: u16 = 0x3F8;

unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val, options(nomem, nostack, preserves_flags));
}

unsafe fn inb(port: u16) -> u8 {
    let val: u8;
    core::arch::asm!("in al, dx", out("al") val, in("dx") port, options(nomem, nostack, preserves_flags));
    val
}

unsafe fn serial_init() {
    outb(COM1 + 1, 0x00); // disable interrupts
    outb(COM1 + 3, 0x80); // DLAB=1 (set baud divisor)
    outb(COM1, 0x03); // divisor lo -> 38400 baud
    outb(COM1 + 1, 0x00); // divisor hi
    outb(COM1 + 3, 0x03); // 8 bits, no parity, 1 stop; DLAB=0
    outb(COM1 + 2, 0xC7); // enable FIFO, clear, 14-byte threshold
    outb(COM1 + 4, 0x0B); // RTS/DSR set
}

unsafe fn serial_print(msg: &[u8]) {
    for &b in msg {
        while inb(COM1 + 5) & 0x20 == 0 {} // wait for THR-empty (LSR bit 5)
        outb(COM1, b);
    }
}

/// UEFI application entry point. The `x86_64-unknown-uefi` target links against the `efi_main`
/// symbol (`/entry:efi_main /subsystem:efi_application`); firmware calls it with the image handle
/// + system table. We print the boot banner via ConOut, bring up the heap, and hand off to the
/// kernel init (envelope-REPL), which diverges.
///
/// HONEST next step (`frame_alloc` v0.2): the heap is a fixed 16 KiB static `BumpAllocator`. A
/// production boot walks the UEFI memory map (`system_table` → `BootServices::GetMemoryMap`) to
/// register real frame regions before `ExitBootServices`; that wiring is the tracked follow-up.
#[no_mangle]
pub extern "efiapi" fn efi_main(
    _image_handle: *mut core::ffi::c_void,
    system_table: *mut core::ffi::c_void,
) -> usize {
    let system_table = system_table as *mut EfiSystemTable;
    unsafe {
        // Serial (headless/QEMU) — guaranteed visible regardless of ConOut routing.
        serial_init();
        serial_print(b"\r\n  ASOLARIA ASI OS   .   kernel 0.2.0-phase3-scaffold   .   booting\r\n");
        serial_print(b"  federation-1024   .   envelope-REPL init   .   E=0   .   fire=0\r\n\r\n");
        // Graphics console (real monitor / GOP) — same banner.
        uefi_print(
            system_table,
            b"\r\n  ASOLARIA ASI OS   .   kernel 0.2.0-phase3-scaffold   .   booting\r\n",
        );
        uefi_print(
            system_table,
            b"  federation-1024   .   envelope-REPL init   .   E=0   .   fire=0\r\n\r\n",
        );
        // Own the display: locate GOP, take the framebuffer, and paint our own boot screen
        // (so the monitor shows ASOLARIA ASI OS, not OVMF's TianoCore logo).
        let st = &*system_table;
        let bs = st.boot_services;
        if !bs.is_null() {
            let mut gop_ptr: *mut core::ffi::c_void = core::ptr::null_mut();
            let status = ((*bs).locate_protocol)(&GOP_GUID, core::ptr::null_mut(), &mut gop_ptr);
            if status == 0 && !gop_ptr.is_null() {
                let gop = gop_ptr as *mut GraphicsOutputProtocol;
                let mode = (*gop).mode;
                if !mode.is_null() && !(*mode).info.is_null() {
                    let info = (*mode).info;
                    let w = (*info).horizontal_resolution;
                    let h = (*info).vertical_resolution;
                    let pps = (*info).pixels_per_scan_line;
                    let fmt = (*info).pixel_format;
                    let fb = (*mode).framebuffer_base as *mut u32;
                    if !fb.is_null() && w >= 320 && h >= 200 {
                        let bg = pack(fmt, 0x05, 0x07, 0x0D);
                        let cyan = pack(fmt, 0x43, 0xE8, 0xD8);
                        let amber = pack(fmt, 0xFF, 0xB4, 0x54);
                        fb_fill(fb, pps, w, h, bg);
                        let text: &[u8] = b"ASOLARIA ASI OS";
                        let scale = if w >= 1024 { 8 } else { 4 };
                        let tw = text.len() as u32 * 9 * scale;
                        let tx = w.saturating_sub(tw) / 2;
                        let ty = h / 2 - 4 * scale;
                        // amber accent bar above the title
                        let bar_top = ty.saturating_sub(3 * scale);
                        for yy in bar_top..(bar_top + scale) {
                            for xx in tx..(tx + tw) {
                                *fb.add((yy * pps + xx) as usize) = amber;
                            }
                        }
                        fb_string(fb, pps, text, tx, ty, scale, cyan);
                    }
                }
            }
        }
    }
    let _anchor = asolaria_kernel_core::FEDERATION_ANCHOR_PID;
    unsafe {
        let heap_ptr = core::ptr::addr_of_mut!(HEAP);
        ALLOCATOR.init(heap_ptr as *mut u8 as usize, HEAP_SIZE);
    }
    // Phase-2 Step 31 — boot to envelope-shell via minimal init system.
    // init::run() returns `!`; as the tail expression its `!` coerces to the efiapi Status
    // return. Firmware never regains control.
    init::run()
}
