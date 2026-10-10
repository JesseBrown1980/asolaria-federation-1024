//! Mouse / touchpad / tablet input through the firmware's pointer protocols.
//!
//! * EFI_SIMPLE_POINTER_PROTOCOL — relative motion (USB/PS2 mouse, some touchpads).
//! * EFI_ABSOLUTE_POINTER_PROTOCOL — absolute position (USB tablet, touch screens).
//!
//! The firmware's console splitter usually republishes every physical pointer as one virtual
//! device on the ConIn handle; polling both the virtual and the physical device would read the
//! same motion twice, so the splitter is preferred when it carries the protocol. Otherwise every
//! handle is polled. What was found is reported (never assumed) on the desktop and over serial.
//!
//! Integer only: relative counts are scaled by the device's own counts-per-mm, absolute
//! positions by the device's range onto the screen.

use crate::ps2mouse::{Probe, Ps2};
use crate::uefi::{Bs, Guid, Handle};

const SIMPLE_GUID: Guid = Guid {
    d1: 0x3187_8c87,
    d2: 0x0b75,
    d3: 0x11d5,
    d4: [0x9a, 0x4f, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
};
const ABS_GUID: Guid = Guid {
    d1: 0x8d59_d32b,
    d2: 0xc655,
    d3: 0x4ae9,
    d4: [0x9b, 0x15, 0xf2, 0x59, 0x04, 0x99, 0x2a, 0x43],
};
/// EFI_PCI_IO_PROTOCOL, used only to find USB host controllers to connect.
const PCI_IO_GUID: Guid = Guid {
    d1: 0x4cf5_b200,
    d2: 0x68b8,
    d3: 0x4ca5,
    d4: [0x9e, 0xec, 0xb2, 0x3e, 0x3f, 0x50, 0x02, 0x9a],
};

#[repr(C)]
struct SimpleMode {
    res_x: u64,
    res_y: u64,
    res_z: u64,
    left: u8,
    right: u8,
}
#[repr(C)]
struct SimpleState {
    dx: i32,
    dy: i32,
    dz: i32,
    left: u8,
    right: u8,
}
#[repr(C)]
struct SimplePointer {
    reset: unsafe extern "efiapi" fn(*mut SimplePointer, u8) -> usize,
    get_state: unsafe extern "efiapi" fn(*mut SimplePointer, *mut SimpleState) -> usize,
    _wait: usize,
    mode: *mut SimpleMode,
}

#[repr(C)]
struct AbsMode {
    min_x: u64,
    min_y: u64,
    min_z: u64,
    max_x: u64,
    max_y: u64,
    max_z: u64,
    attributes: u32,
}
#[repr(C)]
struct AbsState {
    x: u64,
    y: u64,
    z: u64,
    buttons: u32,
}
#[repr(C)]
struct AbsPointer {
    reset: unsafe extern "efiapi" fn(*mut AbsPointer, u8) -> usize,
    get_state: unsafe extern "efiapi" fn(*mut AbsPointer, *mut AbsState) -> usize,
    _wait: usize,
    mode: *mut AbsMode,
}

/// PciIo: only `Pci.Read` (slot 6) is used, to read a controller's class code.
#[repr(C)]
struct PciIo {
    _poll_and_mem_io: [usize; 6],
    pci_read: unsafe extern "efiapi" fn(*mut PciIo, u32, u32, usize, *mut u32) -> usize,
}

const MAX_DEV: usize = 6;

/// What one poll saw.
#[derive(Clone, Copy, Default)]
pub(crate) struct PtrEvent {
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    /// Absolute position on screen, when an absolute device reported.
    pub abs: Option<(i32, i32)>,
    pub left: bool,
    pub right: bool,
}

pub(crate) struct Pointers {
    simple: [*mut SimplePointer; MAX_DEV],
    ns: usize,
    abs: [*mut AbsPointer; MAX_DEV],
    na: usize,
    /// Handle counts as found (physical + virtual), for the on-screen report.
    pub simple_handles: usize,
    pub abs_handles: usize,
    pub via_splitter: bool,
    pub connected_usb: usize,
    /// Physical firmware pointer devices (handles minus the splitter's virtual one).
    pub phys: usize,
    /// The kernel's own PS/2 aux driver, started only when `phys == 0`.
    ps2: Option<Ps2>,
    pub ps2_probe: Option<Probe>,
    left: bool,
    right: bool,
    /// Sub-pixel remainder of relative motion (counts * 16 / res), kept so slow motion isn't lost.
    rem_x: i32,
    rem_y: i32,
}

impl Pointers {
    /// Find pointer devices. `conin` is the system table's ConIn handle (the splitter).
    pub(crate) unsafe fn find(bs: Bs, conin: Handle) -> Pointers {
        let mut p = Pointers {
            simple: [core::ptr::null_mut(); MAX_DEV],
            ns: 0,
            abs: [core::ptr::null_mut(); MAX_DEV],
            na: 0,
            simple_handles: 0,
            abs_handles: 0,
            via_splitter: false,
            connected_usb: 0,
            phys: 0,
            ps2: None,
            ps2_probe: None,
            left: false,
            right: false,
            rem_x: 0,
            rem_y: 0,
        };
        p.scan(bs, conin);
        if p.phys == 0 {
            // No physical device yet: connect drivers to USB host controllers (class 0C03) once
            // and look again. Fast-boot firmware often leaves USB unconnected unless it is the
            // console.
            p.connected_usb = connect_usb(bs);
            if p.connected_usb > 0 {
                p.scan(bs, conin);
            }
        }
        if p.phys == 0 {
            // Still nothing from firmware: start the kernel's own PS/2 aux-port driver.
            let (drv, probe) = Ps2::probe();
            p.ps2 = drv;
            p.ps2_probe = Some(probe);
        }
        for &s in &p.simple[..p.ns] {
            ((*s).reset)(s, 0);
        }
        for &a in &p.abs[..p.na] {
            ((*a).reset)(a, 0);
        }
        p
    }

    unsafe fn scan(&mut self, bs: Bs, conin: Handle) {
        let mut hs = [core::ptr::null_mut(); 16];
        self.simple_handles = bs.handles(&SIMPLE_GUID, &mut hs);
        let sp = bs.handle_protocol(conin, &SIMPLE_GUID) as *mut SimplePointer;
        let mut hs2 = [core::ptr::null_mut(); 16];
        self.abs_handles = bs.handles(&ABS_GUID, &mut hs2);
        let ap = bs.handle_protocol(conin, &ABS_GUID) as *mut AbsPointer;
        self.ns = 0;
        self.na = 0;
        let virt = (!sp.is_null()) as usize + (!ap.is_null()) as usize;
        self.phys = (self.simple_handles + self.abs_handles).saturating_sub(virt);
        if !sp.is_null() || !ap.is_null() {
            self.via_splitter = true;
            if !sp.is_null() {
                self.simple[0] = sp;
                self.ns = 1;
            }
            if !ap.is_null() {
                self.abs[0] = ap;
                self.na = 1;
            }
            return;
        }
        self.via_splitter = false;
        for &h in hs.iter().take(self.simple_handles.min(16)) {
            let s = bs.handle_protocol(h, &SIMPLE_GUID) as *mut SimplePointer;
            if !s.is_null() && self.ns < MAX_DEV {
                self.simple[self.ns] = s;
                self.ns += 1;
            }
        }
        for &h in hs2.iter().take(self.abs_handles.min(16)) {
            let a = bs.handle_protocol(h, &ABS_GUID) as *mut AbsPointer;
            if !a.is_null() && self.na < MAX_DEV {
                self.abs[self.na] = a;
                self.na += 1;
            }
        }
    }

    /// Live pointer sources: (relative, absolute). The splitter alone (no physical device
    /// behind it) counts as nothing; the kernel's PS/2 driver counts as one relative source.
    pub(crate) fn counts(&self) -> (usize, usize) {
        if self.phys == 0 {
            return (self.ps2.is_some() as usize, 0);
        }
        if self.via_splitter {
            // The splitter merges every physical device; report what stands behind it.
            return (self.ns.min(self.phys), self.na.min(self.phys));
        }
        (self.ns, self.na)
    }

    /// Quiesce the kernel PS/2 driver (before the firmware text console owns input).
    pub(crate) unsafe fn suspend(&mut self) {
        if let Some(p) = self.ps2.as_mut() {
            p.suspend();
        }
    }

    pub(crate) unsafe fn resume(&mut self) {
        if let Some(p) = self.ps2.as_mut() {
            p.resume();
        }
    }

    pub(crate) fn ps2_active(&self) -> bool {
        self.ps2.is_some()
    }

    /// Poll every device once. `w`,`h` = screen size for absolute scaling.
    pub(crate) unsafe fn poll(&mut self, w: i32, h: i32) -> Option<PtrEvent> {
        let mut ev = PtrEvent::default();
        let mut got = false;
        let (mut l, mut r) = (self.left, self.right);
        for &s in &self.simple[..self.ns] {
            let mut st = SimpleState {
                dx: 0,
                dy: 0,
                dz: 0,
                left: 0,
                right: 0,
            };
            if ((*s).get_state)(s, &mut st) != 0 {
                continue;
            }
            got = true;
            let (rx, ry) = if (*s).mode.is_null() {
                (0, 0)
            } else {
                ((*(*s).mode).res_x, (*(*s).mode).res_y)
            };
            // Pixels = counts * 16 / counts-per-mm, i.e. ~16 px per mm of travel at the
            // device's stated resolution; a device reporting 0 resolution moves 1 px per count.
            self.rem_x += scale_rel(st.dx, rx);
            self.rem_y += scale_rel(st.dy, ry);
            ev.dz += st.dz;
            l = st.left != 0;
            r = st.right != 0;
        }
        if let Some(ps2) = self.ps2.as_mut() {
            if let Some((dx, dy, pl, pr)) = ps2.poll() {
                got = true;
                self.rem_x += dx * 16;
                self.rem_y += dy * 16;
                l = pl;
                r = pr;
            }
        }
        ev.dx = self.rem_x / 16;
        ev.dy = self.rem_y / 16;
        self.rem_x -= ev.dx * 16;
        self.rem_y -= ev.dy * 16;
        for &a in &self.abs[..self.na] {
            let mut st = AbsState {
                x: 0,
                y: 0,
                z: 0,
                buttons: 0,
            };
            if ((*a).get_state)(a, &mut st) != 0 {
                continue;
            }
            got = true;
            let m = (*a).mode;
            if !m.is_null() {
                let m = &*m;
                ev.abs = Some((
                    scale_abs(st.x, m.min_x, m.max_x, w),
                    scale_abs(st.y, m.min_y, m.max_y, h),
                ));
            }
            l = st.buttons & 1 != 0;
            r = st.buttons & 2 != 0;
        }
        if !got {
            return None;
        }
        self.left = l;
        self.right = r;
        ev.left = l;
        ev.right = r;
        Some(ev)
    }
}

/// Relative counts → sixteenths of a pixel.
fn scale_rel(counts: i32, res: u64) -> i32 {
    if res == 0 || res > 4096 {
        return counts.saturating_mul(16);
    }
    // 16 px per mm, times 16 sub-steps; clamp so one burst can't fling the cursor.
    let v = (counts as i64) * 256 / (res as i64);
    v.clamp(-16 * 400, 16 * 400) as i32
}

/// Absolute device coordinate → screen pixel.
fn scale_abs(v: u64, min: u64, max: u64, span: i32) -> i32 {
    if max <= min || span <= 0 {
        return 0;
    }
    let v = v.clamp(min, max) - min;
    ((v as u128 * (span as u128 - 1)) / (max - min) as u128) as i32
}

/// Connect drivers to every USB host controller (PCI class 0C03), recursively. Returns how
/// many controllers were asked. Read-only PCI config read; the firmware does the binding.
unsafe fn connect_usb(bs: Bs) -> usize {
    let mut hs = [core::ptr::null_mut(); 64];
    let n = bs.handles(&PCI_IO_GUID, &mut hs).min(64);
    let mut asked = 0;
    for &h in &hs[..n] {
        let io = bs.handle_protocol(h, &PCI_IO_GUID) as *mut PciIo;
        if io.is_null() {
            continue;
        }
        let mut class_reg: u32 = 0;
        // Width 2 = UINT32, offset 8 = revision/class dword.
        if ((*io).pci_read)(io, 2, 8, 1, &mut class_reg) != 0 {
            continue;
        }
        if class_reg >> 16 == 0x0C03 {
            let _ = bs.connect_recursive(h);
            asked += 1;
        }
    }
    asked
}
