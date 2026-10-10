//! I2C-HID touchpad driver — the kernel's own driver for laptop touchpads wired to the PCH's
//! Intel Serial IO (Synopsys DesignWare) I2C controller, which is how this Acer's touchpad is
//! connected (MEASURED 2026-10-10 from Windows PnP + the machine's own DSDT):
//!
//! * `\_SB.PCI0.I2C1.TPAD` = Synaptics `SYNA7DB5`, compatible `PNP0C50` (HID over I2C),
//!   parent PCI `8086:A369` at 00:15.1, slave address `0x2C`, HID descriptor register `0x0020`,
//!   400 kHz, interrupt = GpioInt pin 35 on `\_SB.PCI0.GPI0`, level, active low.
//! * The firmware publishes no pointer protocol for it and there is no PS/2 aux mouse, so the
//!   firmware path and the PS/2 driver both see nothing — that is why the cursor never moved.
//!
//! What this driver does (polling, boot services active, no DMA, no interrupts):
//! 1. Finds Intel LPSS I2C controllers on bus 0 (dev 0x15 fn 0..3, dev 0x19 fn 0..2) through
//!    legacy PCI config ports, puts the one it tries into D0, enables memory decode, takes the
//!    LPSS block out of reset and ungates its clock, and checks the DesignWare component type.
//! 2. Tries the known (address, HID-descriptor register) pairs — the measured one first — and
//!    accepts only a well-formed 30-byte HID descriptor (length 30, version 1.00).
//! 3. Reads the report descriptor, finds the Generic-Desktop Mouse application collection
//!    (every PTP touchpad also exposes one, and it is the mode the device starts in), records
//!    where X, Y, wheel and buttons 1/2 sit, then powers the device on and resets it.
//! 4. Each poll reads one input report when the interrupt line says one is pending (GPIO pad
//!    read through the sideband BAR, verified before use) or, when the line cannot be read,
//!    reads blind and drops empty / stale reports.
//!
//! Every step's outcome is kept in `Report` and shown on screen + serial (never assumed).
//! Device writes are limited to the HID-over-I2C SET_POWER(ON) and RESET commands.
//! Integer only. E=0.

use crate::uefi::Bs;
use core::arch::asm;

// ---------------------------------------------------------------- PCI config (ports CF8/CFC)

unsafe fn outl(port: u16, v: u32) {
    asm!("out dx, eax", in("dx") port, in("eax") v, options(nomem, nostack, preserves_flags));
}
unsafe fn inl(port: u16) -> u32 {
    let v: u32;
    asm!("in eax, dx", out("eax") v, in("dx") port, options(nomem, nostack, preserves_flags));
    v
}
fn cfg_addr(dev: u8, fun: u8, off: u8) -> u32 {
    0x8000_0000 | ((dev as u32) << 11) | ((fun as u32) << 8) | (off as u32 & 0xFC)
}
unsafe fn cfg_read(dev: u8, fun: u8, off: u8) -> u32 {
    outl(0xCF8, cfg_addr(dev, fun, off));
    inl(0xCFC)
}
unsafe fn cfg_write(dev: u8, fun: u8, off: u8, v: u32) {
    outl(0xCF8, cfg_addr(dev, fun, off));
    outl(0xCFC, v);
}

// ---------------------------------------------------------------- MMIO

unsafe fn rd(base: u64, off: u32) -> u32 {
    core::ptr::read_volatile((base + off as u64) as *const u32)
}
unsafe fn wr(base: u64, off: u32, v: u32) {
    core::ptr::write_volatile((base + off as u64) as *mut u32, v)
}

// ---------------------------------------------------------------- DesignWare I2C registers

const IC_CON: u32 = 0x00;
const IC_TAR: u32 = 0x04;
const IC_DATA_CMD: u32 = 0x10;
const IC_FS_SCL_HCNT: u32 = 0x1C;
const IC_FS_SCL_LCNT: u32 = 0x20;
const IC_INTR_MASK: u32 = 0x30;
const IC_RAW_INTR_STAT: u32 = 0x34;
const IC_RX_TL: u32 = 0x38;
const IC_TX_TL: u32 = 0x3C;
const IC_CLR_INTR: u32 = 0x40;
const IC_CLR_TX_ABRT: u32 = 0x54;
const IC_CLR_STOP_DET: u32 = 0x60;
const IC_ENABLE: u32 = 0x6C;
const IC_STATUS: u32 = 0x70;
const IC_TXFLR: u32 = 0x74;
const IC_RXFLR: u32 = 0x78;
const IC_SDA_HOLD: u32 = 0x7C;
const IC_TX_ABRT_SOURCE: u32 = 0x80;
const IC_ENABLE_STATUS: u32 = 0x9C;
const IC_COMP_PARAM_1: u32 = 0xF4;
const IC_COMP_TYPE: u32 = 0xFC;
const DW_COMP_TYPE: u32 = 0x4457_0140;

/// Intel LPSS private block (inside BAR0): clock gate and reset control.
const LPSS_CLOCK_PARAMS: u32 = 0x200;
const LPSS_RESETS: u32 = 0x204;
/// FUNC (bits 0..1) | IDMA (bit 2) out of reset — the value Linux intel-lpss writes.
const LPSS_RESETS_RELEASE: u32 = 0x7;

const CMD_READ: u32 = 1 << 8;
const CMD_STOP: u32 = 1 << 9;
const CMD_RESTART: u32 = 1 << 10;
const RAW_TX_ABRT: u32 = 1 << 6;
const RAW_STOP_DET: u32 = 1 << 9;
const STATUS_MST_ACTIVITY: u32 = 1 << 5;

/// IC_CON: master, fast mode (400 kHz class), restart enabled, slave disabled.
const CON_MASTER_FAST: u32 = 0x01 | (2 << 1) | (1 << 5) | (1 << 6);
/// Fast-mode SCL counts. The Serial IO input clock is 120 MHz (SPT) or 216 MHz (CNL); these
/// counts give ~350 kHz at 216 MHz and ~190 kHz at 120 MHz, inside the 400 kHz limits either way
/// (tHIGH >= 0.6 us, tLOW >= 1.3 us).
const FS_HCNT: u32 = 220;
const FS_LCNT: u32 = 400;
/// ~370 ns at 216 MHz, ~670 ns at 120 MHz (fast mode allows 0..0.9 us).
const SDA_HOLD: u32 = 80;

/// (I2C 7-bit address, HID descriptor register). The first pair is this machine's, MEASURED
/// from its DSDT `TPID` table (SYNA7DB5 -> 0x2C, 0x20); the rest are the other touchpads that
/// same table lists (ELAN -> 0x15, 0x01) so a board revision with the other part still works.
const CANDIDATES: [(u8, u16); 2] = [(0x2C, 0x0020), (0x15, 0x0001)];

/// LPSS I2C controller slots, measured slot first (00:15.1 on this Acer). 00:15.0-3 are I2C0-3
/// and 00:19.0-1 are I2C4-5 on Skylake..Alder Lake PCHs; 00:19.2 is a UART there, so it is not
/// listed (the probe resets the LPSS block it tries and must never touch a UART).
const SLOTS: [(u8, u8); 6] = [
    (0x15, 1),
    (0x15, 0),
    (0x15, 2),
    (0x15, 3),
    (0x19, 0),
    (0x19, 1),
];

// ---------------------------------------------------------------- GPIO interrupt line

/// Sideband (P2SB) BAR on Cannon Lake; fixed by firmware. Verified before use: the community's
/// PADBAR register must read back as a plausible pad-config offset.
const SBREG_BAR: u64 = 0xFD00_0000;
/// GPIO community 0 sideband port id (GPP_A, GPP_B) on Cannon Lake PCH-H.
const GPIO_COM0_PID: u64 = 0x6E;
/// ACPI GpioInt pin 35 = GPP_B3 (ACPI numbers GPP_B from 32; GPP_B starts at pad 25 of the
/// community) -> pad 28. CONJECTURE until a metal boot shows the line toggling.
const TP_INT_PAD: u64 = 28;

#[derive(Clone, Copy)]
struct IntLine {
    dw0: u64,
}

impl IntLine {
    unsafe fn find() -> (Option<IntLine>, u32) {
        let comm = SBREG_BAR | (GPIO_COM0_PID << 16);
        let padbar = rd(comm, 0x0C);
        if padbar == 0 || padbar == u32::MAX || !(0x400..0x1000).contains(&padbar) {
            return (None, padbar);
        }
        let dw0 = comm + padbar as u64 + TP_INT_PAD * 16;
        (Some(IntLine { dw0 }), padbar)
    }
    /// Raw pad DW0 (bit 1 = RX state).
    unsafe fn raw(&self) -> u32 {
        rd(self.dw0, 0)
    }
    /// Active low: a report is pending while the pad reads 0.
    unsafe fn pending(&self) -> bool {
        self.raw() & 2 == 0
    }
}

// ---------------------------------------------------------------- probe outcome (shown on screen)

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    NotRun,
    NoController,
    BarUnassigned,
    NotDesignWare,
    NoDevice,
    NoMouseCollection,
    Ready,
}

impl Stage {
    pub(crate) fn text(self) -> &'static [u8] {
        match self {
            Stage::NotRun => b"not_run",
            Stage::NoController => b"no_controller",
            Stage::BarUnassigned => b"bar_unassigned",
            Stage::NotDesignWare => b"not_designware",
            Stage::NoDevice => b"no_device",
            Stage::NoMouseCollection => b"no_mouse_collection",
            Stage::Ready => b"ready",
        }
    }
}

/// Everything the probe saw, for the on-screen / serial report. Integers only.
#[derive(Clone, Copy)]
pub(crate) struct Report {
    pub stage: Stage,
    pub controllers: u8,
    pub dev: u8,
    pub fun: u8,
    pub did: u16,
    pub bar: u64,
    pub comp_type: u32,
    pub pm_was: u8,
    pub addr: u8,
    pub vid: u16,
    pub pid: u16,
    pub rdesc_len: u16,
    pub max_in: u16,
    pub mouse_id: u8,
    pub last_abort: u32,
    pub gpio_padbar: u32,
    pub gpio_dw0: u32,
    pub gpio_used: bool,
}

impl Report {
    const fn new() -> Report {
        Report {
            stage: Stage::NotRun,
            controllers: 0,
            dev: 0,
            fun: 0,
            did: 0,
            bar: 0,
            comp_type: 0,
            pm_was: 0,
            addr: 0,
            vid: 0,
            pid: 0,
            rdesc_len: 0,
            max_in: 0,
            mouse_id: 0,
            last_abort: 0,
            gpio_padbar: 0,
            gpio_dw0: 0,
            gpio_used: false,
        }
    }
}

// ---------------------------------------------------------------- DesignWare master

#[derive(Clone, Copy)]
enum XErr {
    Abort(u32),
    Timeout,
}

struct Dw {
    base: u64,
    depth: u32,
    bs: Bs,
}

impl Dw {
    unsafe fn disable(&self) {
        wr(self.base, IC_ENABLE, 0);
        for _ in 0..100 {
            if rd(self.base, IC_ENABLE_STATUS) & 1 == 0 {
                return;
            }
            self.bs.stall(25);
        }
    }

    unsafe fn init(&self) {
        self.disable();
        wr(self.base, IC_CON, CON_MASTER_FAST);
        wr(self.base, IC_FS_SCL_HCNT, FS_HCNT);
        wr(self.base, IC_FS_SCL_LCNT, FS_LCNT);
        wr(self.base, IC_SDA_HOLD, SDA_HOLD);
        wr(self.base, IC_RX_TL, 0);
        wr(self.base, IC_TX_TL, 0);
        wr(self.base, IC_INTR_MASK, 0);
        let _ = rd(self.base, IC_CLR_INTR);
    }

    unsafe fn target(&self, addr: u8) {
        self.disable();
        wr(self.base, IC_TAR, addr as u32);
        wr(self.base, IC_ENABLE, 1);
    }

    /// One combined transaction: write `w`, then (repeated start) read `r.len()` bytes, STOP.
    unsafe fn xfer(&self, w: &[u8], r: &mut [u8]) -> Result<(), XErr> {
        let b = self.base;
        let _ = rd(b, IC_CLR_INTR);
        let total = w.len() + r.len();
        if total == 0 {
            return Ok(());
        }
        let mut issued = 0usize; // commands pushed (writes + reads)
        let mut got = 0usize; // bytes received
        let mut spins = 0u32;
        while got < r.len() || issued < total {
            let raw = rd(b, IC_RAW_INTR_STAT);
            if raw & RAW_TX_ABRT != 0 {
                let src = rd(b, IC_TX_ABRT_SOURCE);
                let _ = rd(b, IC_CLR_TX_ABRT);
                return Err(XErr::Abort(src));
            }
            let mut progressed = false;
            // Drain received bytes first so the RX FIFO never overflows.
            while got < r.len() && rd(b, IC_RXFLR) > 0 {
                r[got] = rd(b, IC_DATA_CMD) as u8;
                got += 1;
                progressed = true;
            }
            // Push commands while the TX FIFO has room and the reads in flight fit the RX FIFO.
            while issued < total && rd(b, IC_TXFLR) < self.depth {
                let in_flight = issued.saturating_sub(w.len()).saturating_sub(got);
                if issued >= w.len() && in_flight as u32 >= self.depth {
                    break;
                }
                let last = issued + 1 == total;
                let mut cmd = if issued < w.len() {
                    w[issued] as u32
                } else {
                    let mut c = CMD_READ;
                    if issued == w.len() && !w.is_empty() {
                        c |= CMD_RESTART;
                    }
                    c
                };
                if last {
                    cmd |= CMD_STOP;
                }
                wr(b, IC_DATA_CMD, cmd);
                issued += 1;
                progressed = true;
            }
            if !progressed {
                spins += 1;
                if spins > 4000 {
                    return Err(XErr::Timeout);
                }
                self.bs.stall(5);
            }
        }
        // Wait for STOP on the bus (bounded) so the next transaction starts clean.
        for _ in 0..2000 {
            let raw = rd(b, IC_RAW_INTR_STAT);
            if raw & RAW_TX_ABRT != 0 {
                let src = rd(b, IC_TX_ABRT_SOURCE);
                let _ = rd(b, IC_CLR_TX_ABRT);
                return Err(XErr::Abort(src));
            }
            if raw & RAW_STOP_DET != 0 && rd(b, IC_STATUS) & STATUS_MST_ACTIVITY == 0 {
                let _ = rd(b, IC_CLR_STOP_DET);
                return Ok(());
            }
            self.bs.stall(5);
        }
        Err(XErr::Timeout)
    }
}

// ---------------------------------------------------------------- HID report descriptor

#[derive(Clone, Copy, Default)]
struct Field {
    /// Bit offset inside the report (after the report-id byte).
    off: u16,
    size: u8,
    signed: bool,
    present: bool,
}

#[derive(Clone, Copy, Default)]
struct MouseLayout {
    id: u8,
    x: Field,
    y: Field,
    wheel: Field,
    b1: Field,
    b2: Field,
    uses_ids: bool,
}

fn item_value(d: &[u8], size: usize) -> (u32, i32) {
    let mut u = 0u32;
    for (i, &b) in d.iter().enumerate().take(size) {
        u |= (b as u32) << (8 * i);
    }
    let s = match size {
        1 => u as u8 as i8 as i32,
        2 => u as u16 as i16 as i32,
        4 => u as i32,
        _ => 0,
    };
    (u, s)
}

/// Find the first Generic-Desktop/Mouse application collection and where its fields sit.
fn parse_mouse(d: &[u8]) -> Option<MouseLayout> {
    let mut page: u32 = 0;
    let mut lmin: i32 = 0;
    let mut rsize: u32 = 0;
    let mut rcount: u32 = 0;
    let mut rid: u8 = 0;
    let mut usages = [0u32; 16];
    let mut nus = 0usize;
    let mut umin: u32 = 0;
    let mut umax: u32 = 0;
    let mut has_range = false;
    // Global-state stack for Push/Pop.
    let mut stack = [(0u32, 0i32, 0u32, 0u32, 0u8); 4];
    let mut sp = 0usize;
    // Bit offsets per report id (ids 0..=255).
    let mut offs = [0u16; 256];
    let mut depth = 0u32;
    let mut mouse_depth: Option<u32> = None;
    let mut out: Option<MouseLayout> = None;
    let mut uses_ids = false;
    let mut i = 0usize;
    while i < d.len() {
        let p = d[i];
        if p == 0xFE {
            // Long item: skip.
            let n = *d.get(i + 1)? as usize;
            i += 3 + n;
            continue;
        }
        let size = match p & 3 {
            3 => 4,
            n => n as usize,
        };
        let ty = (p >> 2) & 3;
        let tag = p >> 4;
        let data = d.get(i + 1..i + 1 + size)?;
        let (u, s) = item_value(data, size);
        i += 1 + size;
        match (ty, tag) {
            // ---- main
            (0, 0x8) => {
                // Input
                let constant = u & 1 != 0;
                let relative = u & 4 != 0;
                let start = offs[rid as usize];
                if let (Some(_), false) = (mouse_depth, constant) {
                    let lay = out.get_or_insert(MouseLayout {
                        id: rid,
                        ..MouseLayout::default()
                    });
                    if lay.id == rid {
                        for k in 0..rcount {
                            let usage = if has_range {
                                let v = umin + k;
                                if v > umax {
                                    break;
                                }
                                v
                            } else if nus > 0 {
                                usages[(k as usize).min(nus - 1)]
                            } else {
                                break;
                            };
                            let (up, uid) = if usage > 0xFFFF {
                                (usage >> 16, usage & 0xFFFF)
                            } else {
                                (page, usage)
                            };
                            let f = Field {
                                off: start + (k * rsize) as u16,
                                size: rsize.min(32) as u8,
                                signed: lmin < 0,
                                present: true,
                            };
                            match (up, uid) {
                                (1, 0x30) if relative && !lay.x.present => lay.x = f,
                                (1, 0x31) if relative && !lay.y.present => lay.y = f,
                                (1, 0x38) if !lay.wheel.present => lay.wheel = f,
                                (9, 1) if !lay.b1.present => lay.b1 = f,
                                (9, 2) if !lay.b2.present => lay.b2 = f,
                                _ => {}
                            }
                        }
                    }
                }
                offs[rid as usize] = start.wrapping_add((rsize * rcount) as u16);
            }
            (0, 0x9) | (0, 0xB) => {
                // Output / Feature: no input bits.
            }
            (0, 0xA) => {
                // Collection
                depth += 1;
                let cur = if nus > 0 { usages[0] } else { 0 };
                let (up, uid) = if cur > 0xFFFF {
                    (cur >> 16, cur & 0xFFFF)
                } else {
                    (page, cur)
                };
                if u == 1 && up == 1 && uid == 2 && mouse_depth.is_none() && out.is_none() {
                    mouse_depth = Some(depth);
                }
            }
            (0, 0xC) => {
                // End collection
                if mouse_depth == Some(depth) {
                    mouse_depth = None;
                    if out.as_ref().is_some_and(|l| l.x.present && l.y.present) {
                        break;
                    }
                    out = None;
                }
                depth = depth.saturating_sub(1);
            }
            // ---- global
            (1, 0x0) => page = u,
            (1, 0x1) => lmin = s,
            (1, 0x7) => rsize = u,
            (1, 0x8) => {
                rid = u as u8;
                uses_ids = true;
            }
            (1, 0x9) => rcount = u,
            (1, 0xA) => {
                if sp < stack.len() {
                    stack[sp] = (page, lmin, rsize, rcount, rid);
                    sp += 1;
                }
            }
            (1, 0xB) => {
                if sp > 0 {
                    sp -= 1;
                    (page, lmin, rsize, rcount, rid) = stack[sp];
                }
            }
            // ---- local
            (2, 0x0) => {
                if nus < usages.len() {
                    usages[nus] = if size == 4 { u } else { u & 0xFFFF };
                    nus += 1;
                }
            }
            (2, 0x1) => {
                umin = u;
                has_range = true;
            }
            (2, 0x2) => {
                umax = u;
                has_range = true;
            }
            _ => {}
        }
        if ty == 0 {
            // Locals reset after every main item.
            nus = 0;
            has_range = false;
            umin = 0;
            umax = 0;
        }
    }
    let mut lay = out?;
    if !(lay.x.present && lay.y.present) {
        return None;
    }
    lay.uses_ids = uses_ids;
    Some(lay)
}

fn get_bits(r: &[u8], f: Field) -> i32 {
    if !f.present || f.size == 0 {
        return 0;
    }
    let mut v: u32 = 0;
    for k in 0..f.size as usize {
        let bit = f.off as usize + k;
        let byte = r.get(bit / 8).copied().unwrap_or(0);
        if byte >> (bit % 8) & 1 != 0 {
            v |= 1 << k;
        }
    }
    if f.signed && f.size < 32 && v & (1 << (f.size - 1)) != 0 {
        (v | (u32::MAX << f.size)) as i32
    } else {
        v as i32
    }
}

// ---------------------------------------------------------------- the driver

const MAX_IN: usize = 128;
const RDESC_MAX: usize = 1024;

pub(crate) struct I2cHid {
    dw: Dw,
    max_in: usize,
    lay: MouseLayout,
    int: Option<IntLine>,
    last: [u8; MAX_IN],
    last_len: usize,
    repeats: u32,
    left: bool,
    right: bool,
}

/// Bring one controller up; returns the BAR, or the stage that failed.
unsafe fn controller_up(bs: Bs, dev: u8, fun: u8, rep: &mut Report) -> Result<Dw, Stage> {
    rep.dev = dev;
    rep.fun = fun;
    rep.did = (cfg_read(dev, fun, 0) >> 16) as u16;
    // Power state: walk the capability list for PCI PM (id 1), force D0.
    if cfg_read(dev, fun, 0x04) & (1 << 20) != 0 {
        let mut cap = (cfg_read(dev, fun, 0x34) & 0xFC) as u8;
        for _ in 0..16 {
            if cap < 0x40 {
                break;
            }
            let h = cfg_read(dev, fun, cap);
            if h & 0xFF == 1 {
                let pm = cfg_read(dev, fun, cap + 4);
                rep.pm_was = (pm & 3) as u8;
                if pm & 3 != 0 {
                    // D0; keep bit 15 (PME status, write-1-to-clear) untouched.
                    cfg_write(dev, fun, cap + 4, pm & !0x8003);
                    bs.stall(10_000);
                }
                break;
            }
            cap = ((h >> 8) & 0xFC) as u8;
        }
    }
    let lo = cfg_read(dev, fun, 0x10);
    if lo & 1 != 0 {
        return Err(Stage::BarUnassigned);
    }
    let mut bar = (lo & !0xF) as u64;
    if (lo >> 1) & 3 == 2 {
        bar |= (cfg_read(dev, fun, 0x14) as u64) << 32;
    }
    rep.bar = bar;
    if bar == 0 {
        return Err(Stage::BarUnassigned);
    }
    // Memory decode on (bit 1); bus mastering not needed (no DMA).
    let cmd = cfg_read(dev, fun, 0x04);
    if cmd & 2 == 0 {
        cfg_write(dev, fun, 0x04, (cmd & 0xFFFF) | 2);
    }
    // LPSS: reset cycle, then clock on.
    wr(bar, LPSS_RESETS, 0);
    bs.stall(100);
    wr(bar, LPSS_RESETS, LPSS_RESETS_RELEASE);
    let clk = rd(bar, LPSS_CLOCK_PARAMS);
    wr(bar, LPSS_CLOCK_PARAMS, clk | 1);
    bs.stall(100);
    rep.comp_type = rd(bar, IC_COMP_TYPE);
    if rep.comp_type != DW_COMP_TYPE {
        return Err(Stage::NotDesignWare);
    }
    let p1 = rd(bar, IC_COMP_PARAM_1);
    let tx = ((p1 >> 16) & 0xFF) + 1;
    let rx = ((p1 >> 8) & 0xFF) + 1;
    let depth = tx.min(rx).clamp(1, 64);
    let dw = Dw {
        base: bar,
        depth,
        bs,
    };
    dw.init();
    Ok(dw)
}

impl I2cHid {
    /// Probe every LPSS I2C controller for a HID-over-I2C device with a mouse collection.
    pub(crate) unsafe fn probe(bs: Bs) -> (Option<I2cHid>, Report) {
        let mut rep = Report::new();
        let mut int: Option<IntLine> = None;
        let mut stage = Stage::NoController;
        for &(dev, fun) in SLOTS.iter() {
            let id = cfg_read(dev, fun, 0);
            if id & 0xFFFF != 0x8086 {
                continue;
            }
            let class = cfg_read(dev, fun, 0x08) >> 8;
            // Serial bus controller, "other" (0C80xx): Intel Serial IO I2C / SPI / UART.
            if class >> 8 != 0x0C80 {
                continue;
            }
            rep.controllers += 1;
            if rep.controllers == 1 {
                // An Intel Serial IO controller exists, so this is an Intel PCH and the sideband
                // window is decoded; only now read the GPIO community (read-only).
                let (l, padbar) = IntLine::find();
                int = l;
                rep.gpio_padbar = padbar;
                if let Some(l) = int {
                    rep.gpio_dw0 = l.raw();
                }
            }
            let dw = match controller_up(bs, dev, fun, &mut rep) {
                Ok(d) => d,
                Err(s) => {
                    stage = s;
                    continue;
                }
            };
            for &(addr, reg) in CANDIDATES.iter() {
                if let Some(drv) = Self::try_device(&dw, addr, reg, int, &mut rep) {
                    return (Some(drv), rep);
                }
            }
            stage = if rep.vid != 0 {
                Stage::NoMouseCollection
            } else {
                Stage::NoDevice
            };
            dw.disable();
        }
        rep.stage = stage;
        (None, rep)
    }

    unsafe fn try_device(
        dw: &Dw,
        addr: u8,
        reg: u16,
        int: Option<IntLine>,
        rep: &mut Report,
    ) -> Option<I2cHid> {
        dw.target(addr);
        let mut desc = [0u8; 30];
        if let Err(e) = dw.xfer(&reg.to_le_bytes(), &mut desc) {
            if let XErr::Abort(src) = e {
                rep.last_abort = src;
            }
            return None;
        }
        let w = |o: usize| u16::from_le_bytes([desc[o], desc[o + 1]]);
        if w(0) != 30 || w(2) != 0x0100 {
            return None;
        }
        rep.addr = addr;
        rep.vid = w(20);
        rep.pid = w(22);
        let rlen = w(4) as usize;
        let rreg = w(6);
        let in_reg = w(8);
        let max_in = (w(10) as usize).clamp(4, MAX_IN);
        let cmd_reg = w(16);
        rep.rdesc_len = rlen as u16;
        rep.max_in = w(10);
        let _ = in_reg; // input reports are plain reads (HID-over-I2C 1.00 §6.1)
        let mut rd_buf = [0u8; RDESC_MAX];
        let n = rlen.min(RDESC_MAX);
        if dw.xfer(&rreg.to_le_bytes(), &mut rd_buf[..n]).is_err() {
            return None;
        }
        let lay = parse_mouse(&rd_buf[..n])?;
        rep.mouse_id = lay.id;
        // SET_POWER(ON), then RESET; the device answers the reset with an empty input report.
        let c = cmd_reg.to_le_bytes();
        let _ = dw.xfer(&[c[0], c[1], 0x00, 0x08], &mut []);
        dw.bs.stall(2_000);
        let _ = dw.xfer(&[c[0], c[1], 0x00, 0x01], &mut []);
        let mut drv = I2cHid {
            dw: Dw {
                base: dw.base,
                depth: dw.depth,
                bs: dw.bs,
            },
            max_in,
            lay,
            int,
            last: [0; MAX_IN],
            last_len: 0,
            repeats: 0,
            left: false,
            right: false,
        };
        // Wait (bounded, 1 s) for the reset acknowledgement, then drain anything queued.
        for _ in 0..100 {
            dw.bs.stall(10_000);
            if drv.int.map_or(true, |l| l.pending()) {
                let mut b = [0u8; MAX_IN];
                if dw.xfer(&[], &mut b[..max_in]).is_ok() && b[0] == 0 && b[1] == 0 {
                    break;
                }
            }
        }
        // Trust the interrupt line only if it has gone idle (high) after the reset drain;
        // a line that reads "pending" forever is the wrong pad or an inverted reading.
        if let Some(l) = drv.int {
            let mut idle = false;
            for _ in 0..20 {
                if !l.pending() {
                    idle = true;
                    break;
                }
                let mut b = [0u8; MAX_IN];
                let _ = dw.xfer(&[], &mut b[..max_in]);
                dw.bs.stall(1_000);
            }
            rep.gpio_dw0 = l.raw();
            if !idle {
                drv.int = None;
            }
        }
        rep.gpio_used = drv.int.is_some();
        rep.stage = Stage::Ready;
        Some(drv)
    }

    /// One report if pending: (dx, dy, wheel, left, right).
    pub(crate) unsafe fn poll(&mut self) -> Option<(i32, i32, i32, bool, bool)> {
        if let Some(l) = self.int {
            if !l.pending() {
                return None;
            }
        }
        let mut b = [0u8; MAX_IN];
        if self.dw.xfer(&[], &mut b[..self.max_in]).is_err() {
            return None;
        }
        let len = u16::from_le_bytes([b[0], b[1]]) as usize;
        if len <= 2 || len > self.max_in {
            return None;
        }
        let body = &b[2..len];
        // Blind polling only: a byte-identical report repeated many times in a row is the
        // device re-serving its last report, not new motion.
        if self.int.is_none() {
            if len == self.last_len && b[..len] == self.last[..len] {
                self.repeats += 1;
                if self.repeats > 2 {
                    return None;
                }
            } else {
                self.repeats = 0;
                self.last[..len].copy_from_slice(&b[..len]);
                self.last_len = len;
            }
        }
        let rep = if self.lay.uses_ids {
            if body.first().copied() != Some(self.lay.id) {
                return None;
            }
            &body[1..]
        } else {
            body
        };
        let dx = get_bits(rep, self.lay.x);
        let dy = get_bits(rep, self.lay.y);
        let wh = get_bits(rep, self.lay.wheel);
        self.left = get_bits(rep, self.lay.b1) != 0;
        self.right = get_bits(rep, self.lay.b2) != 0;
        Some((dx, dy, wh, self.left, self.right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard boot-protocol mouse descriptor (HID 1.11 Appendix E.10) with a report id.
    const MOUSE: [u8; 52] = [
        0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x85, 0x02, 0x09, 0x01, 0xA1, 0x00, 0x05, 0x09, 0x19,
        0x01, 0x29, 0x03, 0x15, 0x00, 0x25, 0x01, 0x95, 0x03, 0x75, 0x01, 0x81, 0x02, 0x95, 0x01,
        0x75, 0x05, 0x81, 0x01, 0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x15, 0x81, 0x25, 0x7F, 0x75,
        0x08, 0x95, 0x02, 0x81, 0x06, 0xC0, 0xC0,
    ];

    /// PTP-shaped: a Digitizer/Touch Pad collection (id 3, ABSOLUTE 16-bit X/Y) comes first and
    /// must be skipped; then the Mouse collection (id 2) with 8 buttons, 8-bit X/Y and a wheel.
    const PTP_THEN_MOUSE: [u8; 84] = [
        0x05, 0x0D, 0x09, 0x05, 0xA1, 0x01, 0x85, 0x03, // Digitizer / Touch Pad, id 3
        0x05, 0x01, 0x15, 0x00, 0x26, 0xFF, 0x0F, 0x75, 0x10, 0x95, 0x01, // 16-bit abs
        0x09, 0x30, 0x81, 0x02, 0x09, 0x31, 0x81, 0x02, 0xC0, // X, Y absolute; end
        0x05, 0x01, 0x09, 0x02, 0xA1, 0x01, 0x85, 0x02, 0x09, 0x01, 0xA1, 0x00, // Mouse, id 2
        0x05, 0x09, 0x19, 0x01, 0x29, 0x08, 0x15, 0x00, 0x25, 0x01, 0x75, 0x01, 0x95, 0x08, 0x81,
        0x02, // 8 buttons
        0x05, 0x01, 0x09, 0x30, 0x09, 0x31, 0x09, 0x38, 0x15, 0x81, 0x25, 0x7F, 0x75, 0x08, 0x95,
        0x03, 0x81, 0x06, // X, Y, wheel relative
        0xC0, 0xC0, 0x06, 0x00, 0xFF, 0x09, 0x01, 0xA1, 0x01, 0xC0, // vendor tail
    ];

    #[test]
    fn skips_touchpad_collection_finds_mouse() {
        let l = parse_mouse(&PTP_THEN_MOUSE).expect("mouse");
        assert_eq!(l.id, 2);
        assert_eq!((l.b1.off, l.b2.off), (0, 1));
        assert_eq!((l.x.off, l.y.off, l.wheel.off), (8, 16, 24));
        let r = [0x02u8, 0x80, 0x7F, 0xFF]; // right down, dx=-128, dy=127, wheel=-1
        assert_eq!(get_bits(&r, l.x), -128);
        assert_eq!(get_bits(&r, l.y), 127);
        assert_eq!(get_bits(&r, l.wheel), -1);
        assert_eq!((get_bits(&r, l.b1), get_bits(&r, l.b2)), (0, 1));
    }

    #[test]
    fn rejects_touchpad_only() {
        assert!(parse_mouse(&PTP_THEN_MOUSE[..28]).is_none());
        assert!(parse_mouse(&[]).is_none());
        assert!(parse_mouse(&[0x05, 0x01, 0x09]).is_none()); // truncated item
    }

    #[test]
    fn parses_boot_mouse_with_id() {
        let l = parse_mouse(&MOUSE).expect("mouse");
        assert_eq!(l.id, 2);
        assert!(l.uses_ids);
        assert_eq!((l.b1.off, l.b2.off), (0, 1));
        assert_eq!((l.x.off, l.x.size, l.x.signed), (8, 8, true));
        assert_eq!(l.y.off, 16);
        let r = [0x01u8, 0xFB, 0x07]; // left down, dx = -5, dy = +7
        assert_eq!(get_bits(&r, l.x), -5);
        assert_eq!(get_bits(&r, l.y), 7);
        assert_eq!(get_bits(&r, l.b1), 1);
        assert_eq!(get_bits(&r, l.b2), 0);
    }
}
