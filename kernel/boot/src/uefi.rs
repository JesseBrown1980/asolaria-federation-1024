//! The UEFI boot-services surface the desktop needs, declared by hand (no external crate).
//!
//! Function-pointer slots follow the UEFI 2.x Boot Services table order exactly; slots this
//! kernel never calls are padding. Used while boot services are active (the desktop never calls
//! ExitBootServices), so firmware drivers keep serving the screen (GOP), keyboard (ConIn) and
//! pointer devices (Simple/Absolute Pointer).

use crate::EfiTableHeader;
use core::ffi::c_void;

pub(crate) type Handle = *mut c_void;

/// UEFI GUID, laid out as in the spec.
#[repr(C)]
pub(crate) struct Guid {
    pub d1: u32,
    pub d2: u16,
    pub d3: u16,
    pub d4: [u8; 8],
}

/// Boot Services, slots 0..=37 (RaiseTPL .. LocateProtocol).
#[repr(C)]
pub(crate) struct BootServices {
    hdr: EfiTableHeader,
    _tpl: [usize; 4], // 0 RaiseTPL, 1 RestoreTPL, 2 AllocatePages, 3 FreePages
    get_memory_map: unsafe extern "efiapi" fn(*mut usize, *mut u8, *mut usize, *mut usize, *mut u32) -> usize, // 4
    allocate_pool: unsafe extern "efiapi" fn(u32, usize, *mut *mut c_void) -> usize, // 5
    free_pool: unsafe extern "efiapi" fn(*mut c_void) -> usize, // 6
    _ev: [usize; 9], // 7..=15 events + protocol (un)install
    handle_protocol: unsafe extern "efiapi" fn(Handle, *const Guid, *mut *mut c_void) -> usize, // 16
    _mid: [usize; 11], // 17..=27
    stall: unsafe extern "efiapi" fn(usize) -> usize, // 28
    _wd: usize,        // 29 SetWatchdogTimer (the console owns it)
    connect_controller: unsafe extern "efiapi" fn(Handle, *mut Handle, *mut c_void, u8) -> usize, // 30
    _mid2: [usize; 5], // 31..=35
    locate_handle_buffer:
        unsafe extern "efiapi" fn(u32, *const Guid, *mut c_void, *mut usize, *mut *mut Handle) -> usize, // 36
    locate_protocol: unsafe extern "efiapi" fn(*const Guid, *mut c_void, *mut *mut c_void) -> usize, // 37
}

/// EfiLoaderData: pool memory owned by this image.
const LOADER_DATA: u32 = 2;
/// LocateHandleBuffer search type ByProtocol.
const BY_PROTOCOL: u32 = 2;

/// Thin wrapper over a firmware-provided boot-services pointer.
#[derive(Clone, Copy)]
pub(crate) struct Bs(pub *mut BootServices);

impl Bs {
    pub(crate) fn ok(&self) -> bool {
        !self.0.is_null()
    }

    /// Microsecond stall through the firmware.
    pub(crate) unsafe fn stall(&self, us: usize) {
        if self.ok() {
            ((*self.0).stall)(us);
        }
    }

    /// Pool-allocate `n` elements of `T`, filled with `init`. Lives for the rest of boot
    /// (never freed: the desktop owns its buffers until reset).
    pub(crate) unsafe fn alloc_slice<T: Copy>(&self, n: usize, init: T) -> Option<&'static mut [T]> {
        if !self.ok() || n == 0 {
            return None;
        }
        let bytes = n.checked_mul(core::mem::size_of::<T>())?;
        let mut p: *mut c_void = core::ptr::null_mut();
        if ((*self.0).allocate_pool)(LOADER_DATA, bytes, &mut p) != 0 || p.is_null() {
            return None;
        }
        // Pool memory is 8-byte aligned, enough for every T used here (u8/u16/u32/usize).
        if (p as usize) % core::mem::align_of::<T>() != 0 {
            return None;
        }
        let t = p as *mut T;
        for i in 0..n {
            t.add(i).write(init);
        }
        Some(core::slice::from_raw_parts_mut(t, n))
    }

    pub(crate) unsafe fn locate_protocol(&self, g: &Guid) -> *mut c_void {
        if !self.ok() {
            return core::ptr::null_mut();
        }
        let mut p: *mut c_void = core::ptr::null_mut();
        if ((*self.0).locate_protocol)(g, core::ptr::null_mut(), &mut p) != 0 {
            return core::ptr::null_mut();
        }
        p
    }

    pub(crate) unsafe fn handle_protocol(&self, h: Handle, g: &Guid) -> *mut c_void {
        if !self.ok() || h.is_null() {
            return core::ptr::null_mut();
        }
        let mut p: *mut c_void = core::ptr::null_mut();
        if ((*self.0).handle_protocol)(h, g, &mut p) != 0 {
            return core::ptr::null_mut();
        }
        p
    }

    /// Every handle that carries protocol `g`, copied into `out`; returns how many were found
    /// (which may exceed `out.len()`; only the first `out.len()` are copied).
    pub(crate) unsafe fn handles(&self, g: &Guid, out: &mut [Handle]) -> usize {
        if !self.ok() {
            return 0;
        }
        let mut n: usize = 0;
        let mut buf: *mut Handle = core::ptr::null_mut();
        let st = ((*self.0).locate_handle_buffer)(BY_PROTOCOL, g, core::ptr::null_mut(), &mut n, &mut buf);
        if st != 0 || buf.is_null() {
            return 0;
        }
        for (i, o) in out.iter_mut().enumerate().take(n) {
            *o = *buf.add(i);
        }
        ((*self.0).free_pool)(buf as *mut c_void);
        n
    }

    /// Ask the firmware to connect drivers to `h`, recursively (used once, only when no
    /// pointer device was found, so a USB mouse behind an unconnected controller can appear).
    pub(crate) unsafe fn connect_recursive(&self, h: Handle) -> usize {
        if !self.ok() || h.is_null() {
            return usize::MAX;
        }
        ((*self.0).connect_controller)(h, core::ptr::null_mut(), core::ptr::null_mut(), 1)
    }

    /// Sum the UEFI memory map: (conventional_free_bytes, total_mapped_bytes). Integer only.
    pub(crate) unsafe fn memory_totals(&self) -> Option<(u64, u64)> {
        if !self.ok() {
            return None;
        }
        let mut size: usize = 0;
        let mut key: usize = 0;
        let mut dsz: usize = 0;
        let mut ver: u32 = 0;
        // First call reports the size needed (EFI_BUFFER_TOO_SMALL).
        let _ = ((*self.0).get_memory_map)(&mut size, core::ptr::null_mut(), &mut key, &mut dsz, &mut ver);
        if size == 0 {
            return None;
        }
        // Room for the descriptors the allocation itself may add.
        let cap = size + 8 * 64;
        let buf = self.alloc_slice::<u8>(cap, 0)?;
        let mut size = cap;
        let st = ((*self.0).get_memory_map)(&mut size, buf.as_mut_ptr(), &mut key, &mut dsz, &mut ver);
        if st != 0 || dsz < 32 {
            ((*self.0).free_pool)(buf.as_mut_ptr() as *mut c_void);
            return None;
        }
        let mut free = 0u64;
        let mut total = 0u64;
        let mut off = 0usize;
        while off + 32 <= size {
            let d = &buf[off..off + 32];
            let ty = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
            let pages = u64::from_le_bytes([d[24], d[25], d[26], d[27], d[28], d[29], d[30], d[31]]);
            // 7 = EfiConventionalMemory; 0 reserved, 11/12 MMIO, 13 port space are not RAM.
            if ty == 7 {
                free += pages * 4096;
            }
            if !matches!(ty, 0 | 11 | 12 | 13) {
                total += pages * 4096;
            }
            off += dsz;
        }
        ((*self.0).free_pool)(buf.as_mut_ptr() as *mut c_void);
        Some((free, total))
    }
}

impl Bs {
    /// Return pool memory obtained from firmware (e.g. a GOP QueryMode info block).
    pub(crate) unsafe fn free(&self, p: *mut c_void) {
        if self.ok() && !p.is_null() {
            ((*self.0).free_pool)(p);
        }
    }
}
