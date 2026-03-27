use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::Widget,
};

/// Renders a left border rail with optional text content to its right.
///
/// ```text
/// │ line 0
/// │ line 1
/// └ line 2   (last row of total_height)
/// ```
///
/// The `│` / `└` glyph is drawn at `area.x`; each child `Line` is rendered
/// starting at `area.x + 2` (one cell for the border, one for the space).
///
/// `total_height` controls where `└` is placed (last row = `area.y + total_height - 1`).
/// This allows the rail to extend past the children — e.g. when checklist items
/// follow the notes, the rail spans the full detail block but children only
/// cover the notes rows.
///
/// If `total_height` is 0, it defaults to `children.len()`.
pub struct LeftBorderWidget<'a> {
    pub children: Vec<Line<'a>>,
    /// Full height of the rail (where `└` is placed). Defaults to `children.len()`.
    pub total_height: u16,
}

impl<'a> LeftBorderWidget<'a> {
    /// Number of rows this widget will occupy (= `total_height` or `children.len()`).
    pub fn height(&self) -> u16 {
        if self.total_height > 0 {
            self.total_height
        } else {
            self.children.len() as u16
        }
    }
}

impl<'a> Widget for LeftBorderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let style = Style::default().add_modifier(Modifier::DIM);

        let rail_height = self.height();
        let last_y = area.y + rail_height.saturating_sub(1);

        // Draw the rail glyphs for all rail rows.
        for row in 0..rail_height {
            let y = area.y + row;
            let symbol = if y == last_y { "└" } else { "│" };
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol(symbol);
                cell.set_style(style);
            }
        }

        // Render children (may be fewer rows than total_height).
        for (i, child) in self.children.into_iter().enumerate() {
            let y = area.y + i as u16;
            if area.width > 2 {
                let child_area = Rect {
                    x: area.x + 2,
                    y,
                    width: area.width.saturating_sub(2),
                    height: 1,
                };
                child.render(child_area, buf);
            }
        }
    }
}
