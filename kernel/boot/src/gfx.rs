//! Integer 2-D graphics for the desktop: a back buffer in `0x00RRGGBB`, drawing primitives
//! with clipping, and a dirty-rect present to the GOP framebuffer (converting to the firmware's
//! pixel order on the way out). The mouse cursor is drawn on the framebuffer after every
//! present, so the back buffer under it is the save-under: moving the cursor re-presents its old
//! rectangle from the back buffer and redraws it at the new spot.
//!
//! Integer only (no floating point): blending is an integer lerp `(a*(256-t) + b*t) >> 8`.

use crate::font::{glyph16, FONT_H, FONT_W};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub(crate) const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }
    pub(crate) const EMPTY: Rect = Rect::new(0, 0, 0, 0);
    pub(crate) fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
    pub(crate) fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub(crate) fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = (self.x + self.w).max(o.x + o.w);
        let y1 = (self.y + self.h).max(o.y + o.h);
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
    pub(crate) fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        if x1 <= x0 || y1 <= y0 {
            return Rect::EMPTY;
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }
    pub(crate) fn inset(&self, d: i32) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2 * d, self.h - 2 * d)
    }
}

/// `0x00RRGGBB` from components.
pub(crate) const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    (r << 16) | (g << 8) | b
}

/// Integer lerp between two colours, `t` in 0..=256.
pub(crate) fn mix(a: u32, b: u32, t: u32) -> u32 {
    let t = t.min(256);
    let u = 256 - t;
    let ch = |s: u32| ((((a >> s) & 0xFF) * u + ((b >> s) & 0xFF) * t) >> 8) & 0xFF;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// The back buffer (or any off-screen surface): `w*h` pixels, row stride `w`.
pub(crate) struct Canvas {
    pub px: &'static mut [u32],
    pub w: i32,
    pub h: i32,
    clip: Rect,
}

impl Canvas {
    pub(crate) fn new(px: &'static mut [u32], w: i32, h: i32) -> Self {
        Canvas {
            px,
            w,
            h,
            clip: Rect::new(0, 0, w, h),
        }
    }

    pub(crate) fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    /// Restrict drawing to `r` intersected with the CURRENT clip (nested clips only shrink,
    /// so a window body can never draw outside the dirty region it was given); returns the
    /// previous clip for [`Canvas::restore_clip`].
    pub(crate) fn set_clip(&mut self, r: Rect) -> Rect {
        let old = self.clip;
        self.clip = r.intersect(&self.clip);
        old
    }

    pub(crate) fn restore_clip(&mut self, r: Rect) {
        self.clip = r;
    }

    #[inline]
    pub(crate) fn put(&mut self, x: i32, y: i32, c: u32) {
        if self.clip.contains(x, y) {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    pub(crate) fn fill(&mut self, r: Rect, c: u32) {
        let r = r.intersect(&self.clip);
        for y in r.y..r.y + r.h {
            let row = (y * self.w) as usize;
            self.px[row + r.x as usize..row + (r.x + r.w) as usize].fill(c);
        }
    }

    /// Translucent fill (shadows, highlights): every pixel moves `t`/256 toward `c`.
    pub(crate) fn fill_blend(&mut self, r: Rect, c: u32, t: u32) {
        let r = r.intersect(&self.clip);
        for y in r.y..r.y + r.h {
            let row = (y * self.w) as usize;
            for p in &mut self.px[row + r.x as usize..row + (r.x + r.w) as usize] {
                *p = mix(*p, c, t);
            }
        }
    }

    /// Is (dx,dy) inside a rounded corner of radius `rad` placed in a `w`x`h` box?
    fn in_round(dx: i32, dy: i32, w: i32, h: i32, rad: i32) -> bool {
        let cx = if dx < rad {
            rad - 1 - dx
        } else if dx >= w - rad {
            dx - (w - rad)
        } else {
            return true;
        };
        let cy = if dy < rad {
            rad - 1 - dy
        } else if dy >= h - rad {
            dy - (h - rad)
        } else {
            return true;
        };
        // Integer circle test with a half-pixel bias so small radii stay round.
        4 * (cx * cx + cy * cy) + 4 * (cx + cy) < 4 * rad * rad
    }

    /// Pixels to skip at each end of row `dy` of a `w`x`h` box with corner radius `rad`.
    fn round_inset(dy: i32, w: i32, h: i32, rad: i32) -> i32 {
        if dy >= rad && dy < h - rad {
            return 0;
        }
        (0..rad)
            .find(|&dx| Self::in_round(dx, dy, w, h, rad))
            .unwrap_or(rad)
    }

    /// One horizontal span, solid or blended.
    fn span(&mut self, x: i32, y: i32, w: i32, c: u32, t: u32) {
        let r = Rect::new(x, y, w, 1);
        if t >= 256 {
            self.fill(r, c);
        } else {
            self.fill_blend(r, c, t);
        }
    }

    /// Rounded rectangle, solid or translucent (`t` = 256 is solid). Drawn as row spans: only
    /// corner rows pay for the circle test.
    pub(crate) fn round(&mut self, r: Rect, rad: i32, c: u32, t: u32) {
        let rad = rad.min(r.w / 2).min(r.h / 2).max(0);
        let y0 = r.y.max(self.clip.y);
        let y1 = (r.y + r.h).min(self.clip.y + self.clip.h);
        for y in y0..y1 {
            let k = Self::round_inset(y - r.y, r.w, r.h, rad);
            self.span(r.x + k, y, r.w - 2 * k, c, t);
        }
    }

    /// Rounded rectangle with a vertical gradient `top`->`bot`.
    pub(crate) fn round_vgrad(&mut self, r: Rect, rad: i32, top: u32, bot: u32) {
        let rad = rad.min(r.w / 2).min(r.h / 2).max(0);
        let span = (r.h - 1).max(1) as u32;
        let y0 = r.y.max(self.clip.y);
        let y1 = (r.y + r.h).min(self.clip.y + self.clip.h);
        for y in y0..y1 {
            let dy = y - r.y;
            let c = mix(top, bot, (dy as u32 * 256) / span);
            let k = Self::round_inset(dy, r.w, r.h, rad);
            self.span(r.x + k, y, r.w - 2 * k, c, 256);
        }
    }

    /// Plain vertical gradient over `r`.
    pub(crate) fn vgrad(&mut self, r: Rect, top: u32, bot: u32) {
        let span = (r.h - 1).max(1) as u32;
        for dy in 0..r.h {
            let c = mix(top, bot, (dy as u32 * 256) / span);
            self.fill(Rect::new(r.x, r.y + dy, r.w, 1), c);
        }
    }

    /// Bresenham line.
    pub(crate) fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u32) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Thick line: the Bresenham line stamped `t` pixels wide.
    pub(crate) fn line_w(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, t: i32, c: u32) {
        for o in 0..t {
            let d = o - t / 2;
            self.line(x0 + d, y0, x1 + d, y1, c);
            self.line(x0, y0 + d, x1, y1 + d, c);
        }
    }

    /// Filled disc.
    pub(crate) fn disc(&mut self, cx: i32, cy: i32, r: i32, c: u32) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r + r {
                    self.put(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// Ring of thickness `t`.
    pub(crate) fn ring(&mut self, cx: i32, cy: i32, r: i32, t: i32, c: u32) {
        let ri = (r - t).max(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let d = dx * dx + dy * dy;
                if d <= r * r + r && d > ri * ri + ri {
                    self.put(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// Filled convex polygon (scanline, integer edge intersections), solid (`t` = 256) or
    /// translucent. Up to 8 vertices.
    pub(crate) fn fill_convex(&mut self, pts: &[(i32, i32)], c: u32, t: u32) {
        if pts.len() < 3 || pts.len() > 8 {
            return;
        }
        let ymin = pts.iter().map(|p| p.1).min().unwrap_or(0);
        let ymax = pts.iter().map(|p| p.1).max().unwrap_or(0);
        for y in ymin..ymax {
            let mut lo = i32::MAX;
            let mut hi = i32::MIN;
            for i in 0..pts.len() {
                let (ax, ay) = pts[i];
                let (bx, by) = pts[(i + 1) % pts.len()];
                if ay == by {
                    continue;
                }
                let (top, bot) = if ay < by {
                    ((ax, ay), (bx, by))
                } else {
                    ((bx, by), (ax, ay))
                };
                if y < top.1 || y >= bot.1 {
                    continue;
                }
                let x = top.0 + (bot.0 - top.0) * (y - top.1) / (bot.1 - top.1);
                lo = lo.min(x);
                hi = hi.max(x);
            }
            if lo <= hi {
                let r = Rect::new(lo, y, hi - lo + 1, 1);
                if t >= 256 {
                    self.fill(r, c);
                } else {
                    self.fill_blend(r, c, t);
                }
            }
        }
    }

    /// Text in the 8x16 font, integer `scale`. Returns the x after the last glyph.
    pub(crate) fn text(&mut self, x: i32, y: i32, s: &[u8], c: u32, scale: i32) -> i32 {
        let mut cx = x;
        for &ch in s {
            if ch != b' ' {
                let g = glyph16(ch);
                for (row, bits) in g.iter().enumerate() {
                    if *bits == 0 {
                        continue;
                    }
                    for col in 0..8 {
                        if bits & (0x80 >> col) != 0 {
                            if scale == 1 {
                                self.put(cx + col, y + row as i32, c);
                            } else {
                                self.fill(
                                    Rect::new(
                                        cx + col * scale,
                                        y + row as i32 * scale,
                                        scale,
                                        scale,
                                    ),
                                    c,
                                );
                            }
                        }
                    }
                }
            }
            cx += FONT_W * scale;
        }
        cx
    }

    /// Text with a 1-pixel soft shadow (readable over the wallpaper).
    pub(crate) fn text_shadow(&mut self, x: i32, y: i32, s: &[u8], c: u32, scale: i32) -> i32 {
        self.text(x + scale.max(1), y + scale.max(1), s, rgb(0, 0, 0), scale);
        self.text(x, y, s, c, scale)
    }

    /// Text centred horizontally in `[x, x+w)`.
    pub(crate) fn text_center(
        &mut self,
        span: (i32, i32),
        y: i32,
        s: &[u8],
        c: u32,
        scale: i32,
        shadow: bool,
    ) {
        let (x, w) = span;
        let tw = text_w(s, scale);
        let tx = x + (w - tw) / 2;
        if shadow {
            self.text_shadow(tx, y, s, c, scale);
        } else {
            self.text(tx, y, s, c, scale);
        }
    }

    /// Copy `r` from another canvas of the same size (wallpaper cache → back buffer).
    pub(crate) fn copy_from(&mut self, src: &Canvas, r: Rect) {
        let r = r.intersect(&self.clip).intersect(&src.bounds());
        for y in r.y..r.y + r.h {
            let a = (y * self.w + r.x) as usize;
            let b = (y * src.w + r.x) as usize;
            self.px[a..a + r.w as usize].copy_from_slice(&src.px[b..b + r.w as usize]);
        }
    }
}

/// Pixel width of `s` in the 8x16 font at `scale`.
pub(crate) fn text_w(s: &[u8], scale: i32) -> i32 {
    s.len() as i32 * FONT_W * scale
}

/// Line height of the font at `scale`.
pub(crate) const fn line_h(scale: i32) -> i32 {
    FONT_H * scale
}

/// The firmware framebuffer (GOP linear framebuffer, formats 0 = RGBx and 1 = BGRx only).
pub(crate) struct Screen {
    pub fb: *mut u32,
    pub pps: i32,
    pub w: i32,
    pub h: i32,
    pub fmt: u32,
}

// The 12x19 arrow cursor: '#' = outline, '.' = fill.
const CURSOR: [&[u8; 12]; 19] = [
    b"#           ",
    b"##          ",
    b"#.#         ",
    b"#..#        ",
    b"#...#       ",
    b"#....#      ",
    b"#.....#     ",
    b"#......#    ",
    b"#.......#   ",
    b"#........#  ",
    b"#.........# ",
    b"#..........#",
    b"#......#####",
    b"#...#..#    ",
    b"#..##..#    ",
    b"#.#  #..#   ",
    b"##   #..#   ",
    b"#     #..#  ",
    b"      ####  ",
];
pub(crate) const CURSOR_W: i32 = 12;
pub(crate) const CURSOR_H: i32 = 19;

impl Screen {
    #[inline]
    fn conv(&self, c: u32) -> u32 {
        if self.fmt == 0 {
            // PixelRedGreenBlueReserved8BitPerColor: byte0 = R.
            ((c & 0xFF) << 16) | (c & 0xFF00) | ((c >> 16) & 0xFF)
        } else {
            c
        }
    }

    /// Copy `r` of the back buffer to the framebuffer.
    pub(crate) unsafe fn present(&self, src: &Canvas, r: Rect) {
        let r = r.intersect(&Rect::new(0, 0, self.w.min(src.w), self.h.min(src.h)));
        for y in r.y..r.y + r.h {
            let row = self.fb.add((y * self.pps + r.x) as usize);
            let s = &src.px[(y * src.w + r.x) as usize..(y * src.w + r.x + r.w) as usize];
            for (i, &p) in s.iter().enumerate() {
                row.add(i).write_volatile(self.conv(p));
            }
        }
    }

    /// Draw the arrow cursor at (x,y) directly on the framebuffer (`busy` tints it amber).
    pub(crate) unsafe fn cursor(&self, x: i32, y: i32, busy: bool) {
        let outline = self.conv(rgb(0x05, 0x07, 0x0D));
        let fill = self.conv(if busy {
            rgb(0xFF, 0xB4, 0x54)
        } else {
            rgb(0xF4, 0xF8, 0xFF)
        });
        for (dy, row) in CURSOR.iter().enumerate() {
            for (dx, &b) in row.iter().enumerate() {
                let px = x + dx as i32;
                let py = y + dy as i32;
                if b == b' ' || px < 0 || py < 0 || px >= self.w || py >= self.h {
                    continue;
                }
                let c = if b == b'#' { outline } else { fill };
                self.fb.add((py * self.pps + px) as usize).write_volatile(c);
            }
        }
    }
}
