//! The treemap: squarified layout, cushion shading, one mesh.
//!
//! The layout is computed for the current zoom root and size and cached as a
//! single `Arc<Mesh>`, so a frame that changed nothing redraws one shape.
//! Anything that would come out smaller than a couple of pixels is not laid
//! out at all: it is drawn as part of its folder's own area, so the cost of a
//! layout depends on the pixels on screen, not on how many files there are.
//!
//! Colours are by file kind, each the accent's own lightness and chroma with
//! the hue moved (`theme::hued`), clear of 20 to 40 where caution and critical
//! live and clear of the accent's own hue, which is kept for the selection.

use crate::theme::{self, Mode, Palette};
use egui::{Color32, Mesh, Pos2, Rect, pos2};
use heft_scan::kinds::{self, FileKind};
use heft_scan::tree::flag;
use heft_scan::{NodeId, Tree};
use std::sync::Arc;

/// Below this many pixels on a side a cell is not subdivided further.
const MIN_SIDE: f32 = 3.0;

#[derive(Clone, Copy)]
pub struct Cell {
    pub rect: Rect,
    pub node: NodeId,
}

pub struct Layout {
    pub root: NodeId,
    pub area: Rect,
    pub cells: Vec<Cell>,
    pub mesh: Arc<Mesh>,
}

pub fn kind_colour(kind: FileKind, mode: Mode) -> Color32 {
    // Evenly spaced round the wheel, skipping 20 to 40 (caution and
    // critical) and 80 to 120 (the accent, kept for the selection).
    let hue = match kind {
        FileKind::Archive => 55.0,
        FileKind::Data => 135.0,
        FileKind::Code => 160.0,
        FileKind::Build => 185.0,
        FileKind::Image => 210.0,
        FileKind::Document => 235.0,
        FileKind::Video => 260.0,
        FileKind::Font => 285.0,
        FileKind::Audio => 310.0,
        FileKind::Executable => 335.0,
        FileKind::DiskImage => 0.0,
        FileKind::Other => return neutral(mode),
    };
    theme::hued(hue, mode)
}

/// Files with no recognisable kind: a grey on the accent's lightness.
fn neutral(mode: Mode) -> Color32 {
    match mode {
        Mode::Dark => theme::oklch(0.55, 0.0, 0.0),
        Mode::Light => theme::oklch(0.70, 0.0, 0.0),
    }
}

fn node_colour(t: &Tree, n: NodeId, mode: Mode) -> Color32 {
    if t.is_dir(n) {
        // A folder too small to open up: its contents, averaged, as a grey.
        return match mode {
            Mode::Dark => theme::oklch(0.40, 0.0, 0.0),
            Mode::Light => theme::oklch(0.80, 0.0, 0.0),
        };
    }
    kind_colour(kinds::kind_of(t.name_bytes(n)), mode)
}

pub fn build(t: &Tree, root: NodeId, area: Rect, logical: bool, p: Palette, mode: Mode) -> Layout {
    let mut cells = Vec::new();
    lay(t, root, area, logical, &mut cells);
    let mut mesh = Mesh::default();
    for c in &cells {
        cushion(&mut mesh, c.rect, node_colour(t, c.node, mode), p, mode);
    }
    Layout {
        root,
        area,
        cells,
        mesh: Arc::new(mesh),
    }
}

fn lay(t: &Tree, n: NodeId, rect: Rect, logical: bool, out: &mut Vec<Cell>) {
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return;
    }
    if !t.is_dir(n) || rect.width() < MIN_SIDE * 2.0 || rect.height() < MIN_SIDE * 2.0 {
        out.push(Cell { rect, node: n });
        return;
    }
    let kids: Vec<(NodeId, u64)> = t
        .sorted_children(n, logical)
        .into_iter()
        .filter(|&c| !t.has(c, flag::HARDLINK_DUP))
        .map(|c| (c, t.size(c, logical)))
        .filter(|&(_, s)| s > 0)
        .collect();
    let total: u64 = kids.iter().map(|k| k.1).sum();
    if kids.is_empty() || total == 0 {
        out.push(Cell { rect, node: n });
        return;
    }
    // The folder's own size may exceed its children's (a scan in progress),
    // so lay the children out in the share of the rectangle they own.
    let own = t.size(n, logical).max(total);
    let mut inner = rect;
    if own > total {
        let frac = total as f32 / own as f32;
        if rect.width() >= rect.height() {
            inner.set_width(rect.width() * frac);
        } else {
            inner.set_height(rect.height() * frac);
        }
    }
    for (child, r) in squarify(&kids, inner, total) {
        if r.width() < MIN_SIDE || r.height() < MIN_SIDE {
            out.push(Cell {
                rect: r,
                node: child,
            });
        } else {
            lay(t, child, r, logical, out);
        }
    }
}

/// Squarified treemap (Bruls, Huizing, van Wijk). `items` sorted largest
/// first; returns one rectangle per item covering `rect` exactly.
pub fn squarify(items: &[(NodeId, u64)], rect: Rect, total: u64) -> Vec<(NodeId, Rect)> {
    let mut out = Vec::with_capacity(items.len());
    let mut rect = rect;
    let scale = (rect.width() as f64 * rect.height() as f64) / total.max(1) as f64;
    let mut i = 0;
    while i < items.len() {
        let w = rect.width() as f64;
        let h = rect.height() as f64;
        let short = w.min(h).max(1e-6);
        let mut end = i;
        let mut sum = 0.0f64;
        let mut best = f64::INFINITY;
        while end < items.len() {
            let s = items[end].1 as f64 * scale;
            let new_sum = sum + s;
            let rmax = items[i].1 as f64 * scale;
            let rmin = s;
            let worst = (short * short * rmax / (new_sum * new_sum))
                .max(new_sum * new_sum / (short * short * rmin));
            if worst > best && end > i {
                break;
            }
            best = worst;
            sum = new_sum;
            end += 1;
        }
        let last = end == items.len();
        let thick = (sum / short) as f32;
        if w >= h {
            let col_w = if last {
                rect.width()
            } else {
                thick.min(rect.width())
            };
            let mut y = rect.top();
            for (k, &(node, size)) in items[i..end].iter().enumerate() {
                let hh = if k + 1 == end - i {
                    rect.bottom() - y
                } else {
                    (size as f64 * scale / thick as f64) as f32
                };
                out.push((
                    node,
                    Rect::from_min_size(pos2(rect.left(), y), egui::vec2(col_w, hh)),
                ));
                y += hh;
            }
            rect.min.x += col_w;
        } else {
            let row_h = if last {
                rect.height()
            } else {
                thick.min(rect.height())
            };
            let mut x = rect.left();
            for (k, &(node, size)) in items[i..end].iter().enumerate() {
                let ww = if k + 1 == end - i {
                    rect.right() - x
                } else {
                    (size as f64 * scale / thick as f64) as f32
                };
                out.push((
                    node,
                    Rect::from_min_size(pos2(x, rect.top()), egui::vec2(ww, row_h)),
                ));
                x += ww;
            }
            rect.min.y += row_h;
        }
        i = end;
    }
    out
}

/// A cell lit from the top left: a lighter corner, a darker corner, and a
/// one-pixel gap to its neighbours so the map reads as separate things.
fn cushion(mesh: &mut Mesh, r: Rect, base: Color32, p: Palette, mode: Mode) {
    let gap = if r.width() > 4.0 && r.height() > 4.0 {
        0.5
    } else {
        0.0
    };
    let r = r.shrink(gap);
    let (light, dark) = match mode {
        Mode::Dark => (
            lerp(base, p.text_strong, 0.22),
            lerp(base, p.backdrop, 0.38),
        ),
        Mode::Light => (lerp(base, p.popover, 0.35), lerp(base, p.text_strong, 0.22)),
    };
    let i = mesh.vertices.len() as u32;
    mesh.colored_vertex(r.left_top(), light);
    mesh.colored_vertex(r.right_top(), base);
    mesh.colored_vertex(r.right_bottom(), dark);
    mesh.colored_vertex(r.left_bottom(), base);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// The cell under `pos`, if any.
pub fn hit(layout: &Layout, pos: Pos2) -> Option<NodeId> {
    layout
        .cells
        .iter()
        .find(|c| c.rect.contains(pos))
        .map(|c| c.node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squarify_covers_the_rectangle_with_proportional_areas() {
        let items = vec![(0, 600), (1, 300), (2, 60), (3, 30), (4, 10)];
        let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(400.0, 250.0));
        let out = squarify(&items, rect, 1000);
        assert_eq!(out.len(), 5);
        let area: f32 = out.iter().map(|(_, r)| r.area()).sum();
        assert!(
            (area - rect.area()).abs() < 1.0,
            "covers {area} of {}",
            rect.area()
        );
        for ((_, size), (_, r)) in items.iter().zip(&out) {
            let want = rect.area() * *size as f32 / 1000.0;
            assert!(
                (r.area() - want).abs() < want * 0.02 + 1.0,
                "{size}: {} vs {want}",
                r.area()
            );
            assert!(rect.expand(0.01).contains_rect(*r));
        }
    }

    #[test]
    fn no_kind_colour_is_in_the_warning_band_or_the_accent() {
        for k in FileKind::ALL {
            let c = kind_colour(k, Mode::Dark);
            assert_ne!(c, Palette::dark().accent, "{k:?} is the accent");
        }
    }
}
