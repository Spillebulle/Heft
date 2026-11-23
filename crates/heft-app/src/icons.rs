//! The chrome's icons, drawn rather than typed.
//!
//! Lucide's geometry, copied from `lucide-react` 1.43.0 verbatim (one
//! `// lucide <name>` beside each, the name to search lucide.dev for) and
//! flattened by [`crate::lucide`], exactly as Muster does it. A Unicode glyph
//! would be a blank box in Archivo; a hand-drawn mark is the defect §11 of the
//! style guide was written after. Adding one is a paste, never a drawing.

use crate::lucide::{self, Node, Outline};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2, pos2};
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
}

impl Icon {
    pub const ALL: [Self; 20] = [
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
    ];

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
        }
    }
}

static GEOMETRY: LazyLock<Vec<Vec<Outline>>> = LazyLock::new(|| {
    Icon::ALL
        .iter()
        .map(|i| lucide::flatten(i.nodes()))
        .collect()
});

/// Draw `icon` centred in `rect`, in `colour`. The stroke scales with the box.
pub fn draw(painter: &Painter, rect: Rect, icon: Icon, colour: Color32) {
    let size = rect.width().min(rect.height());
    if size <= 1.0 {
        return;
    }
    let scale = size / BOX;
    let origin = rect.center() - Vec2::splat(size * 0.5);
    let width = (2.0 * scale).max(1.0);
    let stroke = Stroke::new(width, colour);
    let at = Icon::ALL
        .iter()
        .position(|i| *i == icon)
        .expect("every icon is in ALL");
    let place = |p: &Pos2| pos2(origin.x + p.x * scale, origin.y + p.y * scale);
    for outline in &GEOMETRY[at] {
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
