//! The mark, drawn rather than stored: the family's rounded square in the
//! accent (Design-Principles §17.4), carrying Heft's glyph, Lucide's
//! `brush-cleaning`, knocked out in `accent_ink`. Ported from Muster's
//! `art.rs`, whose square, blend and window icon these are; only the glyph is
//! Heft's.
//!
//! The window icon is rasterised here at startup, so it cannot drift from the
//! accent; `examples/make-art.rs` writes `assets/icons/` and the installer
//! bitmaps from the same code.

use crate::theme::Palette;
use egui::Color32;

/// A straight RGBA8 buffer, top-left origin, no stride.
///
/// Small enough to be worth having rather than reaching for an image crate: the
/// largest thing drawn through it is a 1354 x 461 banner, and the only
/// operations are "fill" and "blend one pixel".
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, RGBA, **not** premultiplied.
    pub pixels: Vec<u8>,
}

impl Image {
    /// A transparent image.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width as usize) * (height as usize) * 4],
        }
    }

    /// An image filled with one opaque colour.
    pub fn filled(width: u32, height: u32, colour: Color32) -> Self {
        let mut image = Self::new(width, height);
        for px in image.pixels.as_chunks_mut::<4>().0 {
            px.copy_from_slice(&[colour.r(), colour.g(), colour.b(), 255]);
        }
        image
    }

    /// Blends `colour` over the pixel at `(x, y)` with `coverage` in 0..=1.
    ///
    /// Source-over on straight (non-premultiplied) alpha, which is the awkward
    /// case: the destination's colour has to be weighted by its own alpha
    /// before the two are mixed, or drawing onto a transparent ground darkens
    /// the edge towards black. The icons are drawn onto exactly that, so the
    /// wrong version of this is visible as a grey fringe at 16 px.
    pub fn blend(&mut self, x: i32, y: i32, colour: Color32, coverage: f32) {
        if coverage <= 0.0 || x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let alpha = coverage.clamp(0.0, 1.0) * (colour.a() as f32 / 255.0);
        if alpha <= 0.0 {
            return;
        }
        let i = ((y as usize) * (self.width as usize) + x as usize) * 4;
        let dst_a = self.pixels[i + 3] as f32 / 255.0;
        let out_a = alpha + dst_a * (1.0 - alpha);
        let src = [colour.r(), colour.g(), colour.b()];
        for (channel, s) in self.pixels[i..i + 3].iter_mut().zip(src) {
            let s = s as f32 / 255.0;
            let d = *channel as f32 / 255.0;
            let out = (s * alpha + d * dst_a * (1.0 - alpha)) / out_a;
            *channel = (out * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        self.pixels[i + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

/// How round the mark is: 30 % of its side, per §17.4.
const RADIUS_FRACTION: f32 = 0.30;

/// Samples per axis when working out how much of a pixel the shape covers.
///
/// Four means sixteen samples a pixel, which is enough that the curve reads as
/// smooth at 16 px, the size where a coarse edge is most obvious. An analytic
/// coverage would be exact and is not worth the arithmetic for a shape drawn
/// six times at build time and once at startup.
const SAMPLES: i32 = 4;

/// Draws the mark: a rounded square of `side` pixels with its top-left at
/// `(x, y)`, filled with `colour`.
pub fn rounded_square(image: &mut Image, x: f32, y: f32, side: f32, colour: Color32) {
    let radius = side * RADIUS_FRACTION;
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = (x + side).ceil() as i32;
    let y1 = (y + side).ceil() as i32;

    for py in y0..y1 {
        for px in x0..x1 {
            let mut hits = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let fx = px as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let fy = py as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if inside(fx - x, fy - y, side, radius) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let coverage = hits as f32 / (SAMPLES * SAMPLES) as f32;
                image.blend(px, py, colour, coverage);
            }
        }
    }
}

/// Is `(x, y)`, relative to the square's own top-left, inside it?
///
/// The four corners are quarter-circles of `radius`; everywhere else is the
/// square. Written as "fold the point into the nearest corner" rather than as
/// four cases, which is the same test and one branch.
fn inside(x: f32, y: f32, side: f32, radius: f32) -> bool {
    if x < 0.0 || y < 0.0 || x > side || y > side {
        return false;
    }
    let dx = (radius - x).max(x - (side - radius)).max(0.0);
    let dy = (radius - y).max(y - (side - radius)).max(0.0);
    dx * dx + dy * dy <= radius * radius
}

// ---------------------------------------------------------------------------
// The glyph
// ---------------------------------------------------------------------------

/// The glyph's box as a share of the mark's side. Lucide draws inside a 24
/// unit box with about 2 units of air, so 0.68 of the side leaves the brush
/// clear of the rounded corners.
const GLYPH: f32 = 0.68;

/// The stroke, in Lucide units, at large sizes. Lucide's own is 2; the mark
/// is seen small, so it is drawn a little heavier.
const STROKE_UNITS: f32 = 2.25;

/// No stroke thinner than this many pixels: below it the brush antialiases
/// away to a smudge on a taskbar.
const MIN_STROKE_PX: f32 = 1.35;

/// The brush, as segments in the 24-unit box.
fn segments() -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    for o in crate::icons::outlines(crate::icons::Icon::Brush) {
        let pts = &o.points;
        for w in pts.windows(2) {
            out.push((w[0].x, w[0].y, w[1].x, w[1].y));
        }
        if o.closed && pts.len() > 2 {
            let (a, b) = (pts[pts.len() - 1], pts[0]);
            out.push((a.x, a.y, b.x, b.y));
        }
    }
    out
}

/// Draws the brush into a mark of `side` pixels, in `ink`: every subsample
/// within half a stroke of the outline is ink, which gives round caps and
/// joins the way SVG draws Lucide.
fn glyph(image: &mut Image, side: u32, ink: Color32) {
    let s = side as f32;
    let scale = s * GLYPH / 24.0;
    let origin = (s - 24.0 * scale) / 2.0;
    let half = (STROKE_UNITS * scale).max(MIN_STROKE_PX) / 2.0 / scale;
    let segs = segments();
    let near = |x: f32, y: f32| {
        segs.iter().any(|&(x0, y0, x1, y1)| {
            let (dx, dy) = (x1 - x0, y1 - y0);
            let len2 = dx * dx + dy * dy;
            let t = if len2 <= 0.0 {
                0.0
            } else {
                (((x - x0) * dx + (y - y0) * dy) / len2).clamp(0.0, 1.0)
            };
            let (ex, ey) = (x - x0 - t * dx, y - y0 - t * dy);
            ex * ex + ey * ey <= half * half
        })
    };
    for py in 0..side as i32 {
        for px in 0..side as i32 {
            let mut hits = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let fx = px as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let fy = py as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    if near((fx - origin) / scale, (fy - origin) / scale) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                image.blend(px, py, ink, hits as f32 / (SAMPLES * SAMPLES) as f32);
            }
        }
    }
}

/// The mark alone, on transparent, at `side` pixels square: the app icon at
/// every size. The glyph is drawn in the ink that belongs on an accent fill.
pub fn mark(side: u32, palette: Palette) -> Image {
    let mut image = Image::new(side, side);
    rounded_square(&mut image, 0.0, 0.0, side as f32, palette.accent);
    glyph(&mut image, side, palette.accent_ink);
    image
}

/// The window icon, drawn at 64 px; the compositor scales down from it.
pub fn window_icon() -> egui::IconData {
    const SIDE: u32 = 64;
    let image = mark(SIDE, Palette::dark());
    egui::IconData {
        rgba: image.pixels,
        width: SIDE,
        height: SIDE,
    }
}

/// The mark as the chrome draws it (the top bar's square): the same square
/// and glyph through the painter rather than a bitmap.
pub fn paint_mark(painter: &egui::Painter, rect: egui::Rect, palette: Palette) {
    let side = rect.width().min(rect.height());
    painter.rect_filled(rect, side * RADIUS_FRACTION, palette.accent);
    let glyph = egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(side * GLYPH));
    let scale = side * GLYPH / 24.0;
    crate::icons::draw_weighted(
        painter,
        glyph,
        crate::icons::Icon::Brush,
        palette.accent_ink,
        (STROKE_UNITS * scale).max(1.1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(image: &Image, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * image.width + x) * 4) as usize;
        [
            image.pixels[i],
            image.pixels[i + 1],
            image.pixels[i + 2],
            image.pixels[i + 3],
        ]
    }

    #[test]
    fn the_mark_is_the_accent_with_rounded_corners() {
        let p = Palette::dark();
        let image = mark(64, p);
        assert_eq!(px(&image, 0, 0)[3], 0, "the corner is rounded away");
        let edge = px(&image, 32, 1);
        assert_eq!(&edge[..3], &[p.accent.r(), p.accent.g(), p.accent.b()]);
    }

    #[test]
    fn the_glyph_is_drawn_in_accent_ink_at_every_icon_size() {
        let p = Palette::dark();
        let ink = [p.accent_ink.r(), p.accent_ink.g(), p.accent_ink.b()];
        for side in [16u32, 32, 64, 256] {
            let image = mark(side, p);
            let inked = image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|q| {
                    let d: i32 = (0..3).map(|c| (q[c] as i32 - ink[c] as i32).abs()).sum();
                    q[3] > 200 && d < 60
                })
                .count();
            assert!(
                inked > (side * side / 40) as usize,
                "{side} px: {inked} ink pixels"
            );
        }
    }

    #[test]
    fn blending_onto_transparent_keeps_the_colour() {
        let mut image = Image::new(1, 1);
        let red = Color32::from_rgb(255, 0, 0);
        image.blend(0, 0, red, 0.5);
        assert_eq!(&image.pixels[0..3], &[255, 0, 0]);
        assert_eq!(image.pixels[3], 128);
    }

    #[test]
    fn the_window_icon_is_the_mark() {
        let icon = window_icon();
        assert_eq!((icon.width, icon.rgba.len()), (64, 64 * 64 * 4));
    }
}
