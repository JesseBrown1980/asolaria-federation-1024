//! Host bridge on COM2 — how the desktop's Claude terminal reaches Claude Opus 5.5.
//!
//! Parallel mode (ASI OS in QEMU beside Windows): QEMU exposes COM2 as a TCP socket bound to
//! 127.0.0.1 on the host; the host program `asios-claude-bridge` connects to it and answers each
//! question through the operator's own Claude Code harness (`claude -p`, already signed in, with
//! the Asolaria MCPs + FABLE5 hooks). No credential ever enters this kernel or its disk.
//!
//! Bare metal: there is no host behind COM2, and laptops usually have no COM2 decode at all, so
//! the port is never touched unless CPUID reports a hypervisor; the window says why instead.
//!
//! Wire format: ASCII lines, `TAG|payload\n`, the same row shape as the backend `.hbp` journal.
//!   host -> kernel  `HELLO|<who>`  `OUT|<text>`  `END|<status>`  `BUSY|<why>`
//!   kernel -> host  `ASK|<text>`   `PING|`
//! Polled UART, no interrupts. Integer only. E=0.

use crate::{inb, outb};

const COM2: u16 = 0x2F8;
const LINE_MAX: usize = 512;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// Bare metal (no hypervisor): COM2 not probed.
    Metal,
    /// Hypervisor present but no UART answers at COM2.
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
            State::NoPort => b"no COM2 in this VM: start it with asios-desk.sh",
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
        outb(COM2 + 7, 0x5A);
        if inb(COM2 + 7) != 0x5A {
            b.state = State::NoPort;
            return b;
        }
        outb(COM2 + 1, 0x00); // no interrupts
        outb(COM2 + 3, 0x80); // DLAB
        outb(COM2, 0x01); // 115200 baud
        outb(COM2 + 1, 0x00);
        outb(COM2 + 3, 0x03); // 8N1
        outb(COM2 + 2, 0xC7); // FIFO on, cleared
        outb(COM2 + 4, 0x03); // DTR + RTS
        b.state = State::Waiting;
        b.send(b"PING|");
        b
    }

    pub(crate) fn live(&self) -> bool {
        matches!(self.state, State::Waiting | State::Connected)
    }

    /// Write one line (`\n` appended). Bounded wait per byte; drops on a stuck port.
    pub(crate) unsafe fn send(&mut self, s: &[u8]) {
        if !self.live() {
            return;
        }
        for &c in s.iter().chain(b"\n".iter()) {
            let mut spins = 0u32;
            while inb(COM2 + 5) & 0x20 == 0 {
                spins += 1;
                if spins > 100_000 {
                    return;
                }
            }
            outb(COM2, c);
        }
    }

    /// Send a question typed in the Claude window.
    pub(crate) unsafe fn ask(&mut self, q: &[u8]) {
        let mut row = [0u8; 8 + 128];
        row[..4].copy_from_slice(b"ASK|");
        let n = q.len().min(128);
        row[4..4 + n].copy_from_slice(&q[..n]);
        self.send(&row[..4 + n]);
        self.asked = self.asked.wrapping_add(1);
    }

    /// Drain received bytes; returns the next complete line, if any (bounded work per call).
    pub(crate) unsafe fn poll(&mut self) -> Option<Msg<'_>> {
        if !self.live() {
            return None;
        }
        self.ready = false;
        for _ in 0..1024 {
            if inb(COM2 + 5) & 0x01 == 0 {
                break;
            }
            let c = inb(COM2);
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
