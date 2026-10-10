//! Host bridge on COM3 — how the desktop's Claude terminal reaches Claude Opus 5.5.
//!
//! Parallel mode (ASI OS in QEMU beside Windows): the host program `asios-claude-bridge`
//! launches the VM itself and owns COM3 through the VM process's stdio (no socket, nothing any
//! other process can connect to). It answers each question through the operator's own Claude
//! Code harness (`claude -p`, already signed in, with the Asolaria MCPs + FABLE5 hooks).
//! No credential ever enters this kernel or its disk.
//!
//! Bare metal: there is no host behind COM3, and laptops usually have no COM3 decode at all, so
//! the port is never touched unless CPUID reports a hypervisor; the window says why instead.
//!
//! Wire format: ASCII lines, `TAG|payload\n`, the same row shape as the backend `.hbp` journal.
//!   host -> kernel  `HELLO|<who>`  `OUT|<text>`  `END|<status>`  `BUSY|<why>`
//!   kernel -> host  `ASK|<text>`   `PING|`
//! Polled UART, no interrupts. Integer only. E=0.

use crate::{inb, outb};

/// COM3 (0x3E8): OVMF enumerates only COM1/COM2 (0x3F8/0x2F8) and attaches its own terminal
/// to them, which stole the host's bytes as keystrokes (MEASURED 2026-10-10). The firmware never
/// sees a UART at 0x3E8, so the kernel owns it alone and no boot text leaks into the channel.
const COM3: u16 = 0x3E8;
const LINE_MAX: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// Bare metal (no hypervisor): COM3 not probed.
    Metal,
    /// Hypervisor present but no UART answers at COM3.
    NoPort,
    /// Port present, no HELLO from the host bridge yet.
    Waiting,
    /// HELLO received.
    Connected,
}

impl State {
    pub(crate) fn text(self) -> &'static [u8] {
        match self {
            State::Metal => b"bare metal: run ASI OS in parallel mode for the Claude bridge",
            State::NoPort => b"no COM3 in this VM: start ASI OS with START-ASOLARIA-ASI-OS",
            State::Waiting => b"waiting for the host bridge (asios-claude-bridge)",
            State::Connected => b"bridge connected",
        }
    }
}

/// One complete line received from the host.
pub(crate) enum Msg<'a> {
    Hello(&'a [u8]),
    Out(&'a [u8]),
    End(&'a [u8]),
    Busy(&'a [u8]),
    Other,
}

pub(crate) struct Bridge {
    pub state: State,
    line: [u8; LINE_MAX],
    n: usize,
    ready: bool,
    /// Bytes of the completed line (valid while `ready`).
    done: [u8; LINE_MAX],
    dn: usize,
    pub asked: u32,
    pub answered: u32,
}

/// CPUID.1:ECX bit 31 = running under a hypervisor.
fn hypervisor() -> bool {
    // SAFETY: CPUID leaf 1 exists on every x86_64 CPU.
    let r = unsafe { core::arch::x86_64::__cpuid(1) };
    r.ecx & (1 << 31) != 0
}

impl Bridge {
    pub(crate) unsafe fn open() -> Bridge {
        let mut b = Bridge {
            state: State::Metal,
            line: [0; LINE_MAX],
            n: 0,
            ready: false,
            done: [0; LINE_MAX],
            dn: 0,
            asked: 0,
            answered: 0,
        };
        if !hypervisor() {
            return b;
        }
        // Scratch-register probe: a 16550 keeps what is written to offset 7.
        outb(COM3 + 7, 0x5A);
        if inb(COM3 + 7) != 0x5A {
            b.state = State::NoPort;
            return b;
        }
        outb(COM3 + 1, 0x00); // no interrupts
        outb(COM3 + 3, 0x80); // DLAB
        outb(COM3, 0x01); // 115200 baud
        outb(COM3 + 1, 0x00);
        outb(COM3 + 3, 0x03); // 8N1
        outb(COM3 + 2, 0xC7); // FIFO on, cleared
        outb(COM3 + 4, 0x03); // DTR + RTS
        b.state = State::Waiting;
        let _ = b.send(b"PING|");
        b
    }

    pub(crate) fn live(&self) -> bool {
        matches!(self.state, State::Waiting | State::Connected)
    }

    /// Write one line (`\n` appended). Bounded wait per byte. Returns false if the port stuck;
    /// then a best-effort `\n` ends the partial line so the host drops it as a fragment rather
    /// than joining it onto the next line (opus55 F7).
    pub(crate) unsafe fn send(&mut self, s: &[u8]) -> bool {
        if !self.live() {
            return false;
        }
        for &c in s.iter().chain(b"\n".iter()) {
            if !Self::tx_ready() {
                if Self::tx_ready() {
                    outb(COM3, b'\n');
                }
                return false;
            }
            outb(COM3, c);
        }
        true
    }

    unsafe fn tx_ready() -> bool {
        for _ in 0..100_000 {
            if inb(COM3 + 5) & 0x20 != 0 {
                return true;
            }
        }
        false
    }

    /// Send a question typed in the Claude window.
    pub(crate) unsafe fn ask(&mut self, q: &[u8]) -> bool {
        let mut row = [0u8; 8 + 128];
        row[..4].copy_from_slice(b"ASK|");
        let n = q.len().min(128);
        row[4..4 + n].copy_from_slice(&q[..n]);
        let ok = self.send(&row[..4 + n]);
        if ok {
            self.asked = self.asked.wrapping_add(1);
        }
        ok
    }

    /// Drain received bytes; returns the next complete line, if any (bounded work per call).
    pub(crate) unsafe fn poll(&mut self) -> Option<Msg<'_>> {
        if !self.live() {
            return None;
        }
        self.ready = false;
        for _ in 0..1024 {
            if inb(COM3 + 5) & 0x01 == 0 {
                break;
            }
            let c = inb(COM3);
            if c == b'\n' {
                self.done[..self.n].copy_from_slice(&self.line[..self.n]);
                self.dn = self.n;
                self.n = 0;
                self.ready = true;
                break;
            }
            if c == b'\r' {
                continue;
            }
            if self.n < LINE_MAX {
                self.line[self.n] = if (0x20..0x7F).contains(&c) { c } else { b'?' };
                self.n += 1;
            }
        }
        if !self.ready {
            return None;
        }
        let l = &self.done[..self.dn];
        let (tag, body) = match l.iter().position(|&c| c == b'|') {
            Some(p) => (&l[..p], &l[p + 1..]),
            None => (l, &l[l.len()..]),
        };
        Some(match tag {
            b"HELLO" => {
                self.state = State::Connected;
                Msg::Hello(body)
            }
            b"OUT" => Msg::Out(body),
            b"END" => {
                self.answered = self.answered.wrapping_add(1);
                Msg::End(body)
            }
            b"BUSY" => Msg::Busy(body),
            _ => Msg::Other,
        })
    }
}
