//! The chrome's icons, drawn rather than typed.
//!
//! Lucide's geometry, copied from `lucide-react` 1.43.0 verbatim (one
//! `// lucide <name>` beside each, the name to search lucide.dev for) and
//! flattened by [`crate::lucide`], exactly as Muster does it. A Unicode glyph
//! would be a blank box in Archivo; a hand-drawn mark is the defect §11 of the
//! style guide was written after. Adding one is a paste, never a drawing.

use crate::lucide::{self, Node, Outline};
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2};
use std::sync::LazyLock;

const BOX: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Drive,
    Folder,
    File,
    Trash,
    Refresh,
    Close,
    ChevronRight,
    ChevronDown,
    Tree,
    Treemap,
    Sunburst,
    Types,
    Age,
    Duplicates,
    Suggestions,
    Gear,
    AddFolder,
    Up,
    Package,
    Info,
    Brush,
    Github,
}

impl Icon {
    pub const ALL: [Self; 22] = [
        Self::Drive,
        Self::Folder,
        Self::File,
        Self::Trash,
        Self::Refresh,
        Self::Close,
        Self::ChevronRight,
        Self::ChevronDown,
        Self::Tree,
        Self::Treemap,
        Self::Sunburst,
        Self::Types,
        Self::Age,
        Self::Duplicates,
        Self::Suggestions,
        Self::Gear,
        Self::AddFolder,
        Self::Up,
        Self::Package,
        Self::Info,
        Self::Brush,
        Self::Github,
    ];

    /// Is this mark filled rather than stroked? GitHub's is: a logo is drawn
    /// as itself (Muster's rule), and everything else is a Lucide outline.
    fn is_filled(self) -> bool {
        matches!(self, Self::Github)
    }

    fn nodes(self) -> &'static [Node] {
        match self {
            // lucide hard-drive
            Self::Drive => &[
                Node::Path("M10 16h.01"),
                Node::Path(
                    "M2.212 11.577a2 2 0 0 0-.212.896V18a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-5.527a2 2 0 0 0-.212-.896L18.55 5.11A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z",
                ),
                Node::Path("M21.946 12.013H2.054"),
                Node::Path("M6 16h.01"),
            ],
            // lucide folder
            Self::Folder => &[Node::Path(
                "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
            )],
            // lucide file
            Self::File => &[
                Node::Path(
                    "M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z",
                ),
                Node::Path("M14 2v5a1 1 0 0 0 1 1h5"),
            ],
            // lucide trash
            Self::Trash => &[
                Node::Path("M10 11v6"),
                Node::Path("M14 11v6"),
                Node::Path("M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"),
                Node::Path("M3 6h18"),
                Node::Path("M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"),
            ],
            // lucide refresh-cw
            Self::Refresh => &[
                Node::Path("M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8"),
                Node::Path("M21 3v5h-5"),
                Node::Path("M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16"),
                Node::Path("M8 16H3v5"),
            ],
            // lucide x
            Self::Close => &[Node::Path("M18 6 6 18"), Node::Path("m6 6 12 12")],
            // lucide chevron-right
            Self::ChevronRight => &[Node::Path("m9 18 6-6-6-6")],
            // lucide chevron-down
            Self::ChevronDown => &[Node::Path("m6 9 6 6 6-6")],
            // lucide list-tree
            Self::Tree => &[
                Node::Path("M8 5h13"),
                Node::Path("M13 12h8"),
                Node::Path("M13 19h8"),
                Node::Path("M3 10a2 2 0 0 0 2 2h3"),
                Node::Path("M3 5v12a2 2 0 0 0 2 2h3"),
            ],
            // lucide layout-grid
            Self::Treemap => &[
                Node::Rect {
                    x: 3.0,
                    y: 3.0,
                    w: 7.0,
                    h: 7.0,
                    r: 1.0,
                },
                Node::Rect {
                    x: 14.0,
                    y: 3.0,
                    w: 7.0,
                    h: 7.0,
                    r: 1.0,
                },
                Node::Rect {
                    x: 14.0,
                    y: 14.0,
                    w: 7.0,
                    h: 7.0,
                    r: 1.0,
                },
                Node::Rect {
                    x: 3.0,
                    y: 14.0,
                    w: 7.0,
                    h: 7.0,
                    r: 1.0,
                },
            ],
            // lucide chart-pie
            Self::Sunburst => &[
                Node::Path(
                    "M21 12c.552 0 1.005-.449.95-.998a10 10 0 0 0-8.953-8.951c-.55-.055-.998.398-.998.95v8a1 1 0 0 0 1 1z",
                ),
                Node::Path("M21.21 15.89A10 10 0 1 1 8 2.83"),
            ],
            // lucide files
            Self::Types => &[
                Node::Path("M15 2h-4a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2V8"),
                Node::Path(
                    "M16.706 2.706A2.4 2.4 0 0 0 15 2v5a1 1 0 0 0 1 1h5a2.4 2.4 0 0 0-.706-1.706z",
                ),
                Node::Path("M5 7a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h8a2 2 0 0 0 1.732-1"),
            ],
            // lucide clock
            Self::Age => &[
                Node::Circle {
                    cx: 12.0,
                    cy: 12.0,
                    r: 10.0,
                },
                Node::Path("M12 6v6l4 2"),
            ],
            // lucide copy
            Self::Duplicates => &[
                Node::Rect {
                    x: 8.0,
                    y: 8.0,
                    w: 14.0,
                    h: 14.0,
                    r: 2.0,
                },
                Node::Path("M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"),
            ],
            // lucide lightbulb
            Self::Suggestions => &[
                Node::Path(
                    "M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5",
                ),
                Node::Path("M9 18h6"),
                Node::Path("M10 22h4"),
            ],
            // lucide settings
            Self::Gear => &[
                Node::Path(
                    "M9.671 4.136a2.34 2.34 0 0 1 4.659 0 2.34 2.34 0 0 0 3.319 1.915 2.34 2.34 0 0 1 2.33 4.033 2.34 2.34 0 0 0 0 3.831 2.34 2.34 0 0 1-2.33 4.033 2.34 2.34 0 0 0-3.319 1.915 2.34 2.34 0 0 1-4.659 0 2.34 2.34 0 0 0-3.32-1.915 2.34 2.34 0 0 1-2.33-4.033 2.34 2.34 0 0 0 0-3.831A2.34 2.34 0 0 1 6.35 6.051a2.34 2.34 0 0 0 3.319-1.915",
                ),
                Node::Circle {
                    cx: 12.0,
                    cy: 12.0,
                    r: 3.0,
                },
            ],
            // lucide folder-plus
            Self::AddFolder => &[
                Node::Path("M12 10v6"),
                Node::Path("M9 13h6"),
                Node::Path(
                    "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z",
                ),
            ],
            // lucide arrow-up
            Self::Up => &[Node::Path("m5 12 7-7 7 7"), Node::Path("M12 19V5")],
            // lucide package
            Self::Package => &[
                Node::Path(
                    "M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z",
                ),
                Node::Path("M12 22V12"),
                Node::Path("M3.29 7L12 12L20.71 7"),
                Node::Path("m7.5 4.27 9 5.15"),
            ],
            // lucide info
            Self::Info => &[
                Node::Circle {
                    cx: 12.0,
                    cy: 12.0,
                    r: 10.0,
                },
                Node::Path("M12 16v-4"),
                Node::Path("M12 8h.01"),
            ],
            // lucide brush-cleaning
            Self::Brush => &[
                Node::Path("m16 22-1-4"),
                Node::Path(
                    "M19 14a1 1 0 0 0 1-1v-1a2 2 0 0 0-2-2h-3a1 1 0 0 1-1-1V4a2 2 0 0 0-4 0v5a1 1 0 0 1-1 1H6a2 2 0 0 0-2 2v1a1 1 0 0 0 1 1",
                ),
                Node::Path("M19 14H5l-1.973 6.767A1 1 0 0 0 4 22h16a1 1 0 0 0 .973-1.233z"),
                Node::Path("m8 22 1-4"),
            ],
            // Simple Icons `github`, copied verbatim the way the Lucide paths
            // above are. It is one closed subpath and it is filled, not
            // stroked; see [`Icon::is_filled`].
            Self::Github => &[Node::Path(
                "M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12",
            )],
        }
    }
}

/// Outlines per icon, in the 24-unit box, and for the filled mark the
/// triangles that cover them (egui fans a closed path from its first point,
/// which paints a notched shape's gaps shut). Worked out once.
struct Drawn {
    outlines: Vec<Outline>,
    fill: Vec<Vec<[u32; 3]>>,
}

static GEOMETRY: LazyLock<Vec<Drawn>> = LazyLock::new(|| {
    Icon::ALL
        .iter()
        .map(|i| {
            let outlines = lucide::flatten(i.nodes());
            let fill = if i.is_filled() {
                outlines.iter().map(|o| triangulate(&o.points)).collect()
            } else {
                Vec::new()
            };
            Drawn { outlines, fill }
        })
        .collect()
});

/// The icon's flattened outlines in the 24-unit box (the mark rasterises
/// these into the app icon).
pub fn outlines(icon: Icon) -> &'static [Outline] {
    let at = Icon::ALL.iter().position(|i| *i == icon).expect("in ALL");
    &GEOMETRY[at].outlines
}

/// Draw `icon` centred in `rect`, in `colour`. The stroke scales with the box.
pub fn draw(painter: &Painter, rect: Rect, icon: Icon, colour: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 1.0 {
        return;
    }
    let scale = size / BOX;
    let origin = rect.center() - Vec2::splat(size * 0.5);
    draw_with(painter, origin, scale, (2.0 * scale).max(1.0), icon, colour);
}

/// `draw` with the stroke width chosen by the caller: the mark's glyph is
/// drawn heavier than the chrome's icons so it holds at 16 px.
pub fn draw_weighted(painter: &Painter, rect: Rect, icon: Icon, colour: Color32, width: f32) {
    let size = rect.width().min(rect.height());
    if size <= 1.0 {
        return;
    }
    let scale = size / BOX;
    let origin = rect.center() - Vec2::splat(size * 0.5);
    draw_with(painter, origin, scale, width, icon, colour);
}

fn draw_with(painter: &Painter, origin: Pos2, scale: f32, width: f32, icon: Icon, colour: Color32) {
    let stroke = Stroke::new(width, colour);
    let at = Icon::ALL
        .iter()
        .position(|i| *i == icon)
        .expect("every icon is in ALL");
    let drawn = &GEOMETRY[at];
    let place = |p: &Pos2| pos2(origin.x + p.x * scale, origin.y + p.y * scale);
    if !drawn.fill.is_empty() {
        for (outline, triangles) in drawn.outlines.iter().zip(&drawn.fill) {
            let points: Vec<_> = outline.points.iter().map(place).collect();
            let mut mesh = Mesh::default();
            for point in &points {
                mesh.colored_vertex(*point, colour);
            }
            for [a, b, c] in triangles {
                mesh.add_triangle(*a, *b, *c);
            }
            painter.add(Shape::mesh(mesh));
            // A mesh has hard edges; a hairline along the outline is what
            // antialiases the silhouette.
            painter.add(Shape::closed_line(points, Stroke::new(1.0_f32, colour)));
        }
        return;
    }
    for outline in &drawn.outlines {
        let points: Vec<_> = outline.points.iter().map(place).collect();
        if outline.is_dot() {
            painter.circle_filled(points[0], width / 2.0, colour);
        } else if outline.closed {
            painter.add(Shape::closed_line(points, stroke));
        } else {
            painter.add(Shape::line(points, stroke));
        }
    }
}

/// Twice the area a run of points encloses, signed by which way it winds.
///
/// The sign is the whole point: it says which turn is a corner and which is a
/// notch, and in a y-down box that is the opposite of what a mathematician
/// would write down. Nothing here cares which way round it is, only that one
/// answer is used throughout.
fn winding(points: &[Pos2]) -> f32 {
    let mut sum = 0.0;
    for (index, a) in points.iter().enumerate() {
        let b = points[(index + 1) % points.len()];
        sum += a.x * b.y - b.x * a.y;
    }
    sum
}

/// Which side of the line `a`→`b` the point `c` falls.
fn turn(a: Pos2, b: Pos2, c: Pos2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Is `p` inside the triangle `a`, `b`, `c`, which winds positively?
fn inside(a: Pos2, b: Pos2, c: Pos2, p: Pos2) -> bool {
    turn(a, b, p) > 0.0 && turn(b, c, p) > 0.0 && turn(c, a, p) > 0.0
}

/// Cover a closed outline with triangles, by ear clipping.
///
/// **egui cannot fill this shape and does not say so.** Its tessellator covers
/// a closed path with a fan from the first point, which is right for the convex
/// shapes the widgets are made of and quietly wrong for anything with a notch
/// in it — the GitHub mark filled that way is a solid blob with its own arms
/// painted across the gaps. So the one filled mark is handed to the painter as
/// a mesh, and this is what works the mesh out: take the corner that no other
/// point of the outline sits inside, cut it off, and go round again.
///
/// Quadratic in the number of points and run once for the life of the process,
/// on one icon of some hundreds of points. The answer is indices into `points`,
/// so it holds for every size that icon is ever drawn at.
fn triangulate(points: &[Pos2]) -> Vec<[u32; 3]> {
    /// Below this an ear is a sliver rather than a triangle. The box is 24
    /// units across, so this is a millionth of it.
    const SLIVER: f32 = 1e-5;

    if points.len() < 3 {
        return Vec::new();
    }
    let mut left: Vec<u32> = (0..points.len() as u32).collect();
    // Ears are cut from a positively wound outline, so an outline that winds
    // the other way is walked backwards instead of being copied.
    if winding(points) < 0.0 {
        left.reverse();
    }

    let mut out = Vec::with_capacity(points.len() - 2);
    let at = |index: u32| points[index as usize];
    while left.len() > 3 {
        let before = left.len();
        let mut i = 0;
        while i < left.len() && left.len() > 3 {
            let n = left.len();
            let (a, b, c) = (left[(i + n - 1) % n], left[i], left[(i + 1) % n]);
            let corner = turn(at(a), at(b), at(c));
            // A corner flat enough to have no area is cut without asking what
            // is inside it: removing a point that lies on the line between its
            // neighbours cannot change the shape, and leaving it is how a walk
            // finds no ear at all and stalls.
            let ear = if corner.abs() < SLIVER {
                true
            } else {
                corner > 0.0
                    && !left.iter().any(|&other| {
                        other != a
                            && other != b
                            && other != c
                            && inside(at(a), at(b), at(c), at(other))
                    })
            };
            if ear {
                if corner.abs() >= SLIVER {
                    out.push([a, b, c]);
                }
                left.remove(i);
            } else {
                i += 1;
            }
        }
        // A whole pass without an ear means the outline crosses itself, which
        // no icon here does. Fanning what is left keeps the mark on screen
        // rather than dropping it, and the test below is what would catch it.
        if left.len() == before {
            break;
        }
    }
    for k in 1..left.len().saturating_sub(1) {
        out.push([left[0], left[k], left[k + 1]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_flattens_to_something_inside_its_box() {
        for icon in Icon::ALL {
            let outlines = lucide::flatten(icon.nodes());
            assert!(!outlines.is_empty(), "{icon:?} drew nothing");
            for o in &outlines {
                for p in &o.points {
                    assert!(
                        (-0.5..=24.5).contains(&p.x) && (-0.5..=24.5).contains(&p.y),
                        "{icon:?} leaves its box"
                    );
                }
            }
        }
    }
}
