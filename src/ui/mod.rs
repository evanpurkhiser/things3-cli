pub mod widgets;
pub mod views;

use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

/// Render a [`Widget`] into a plain string.
///
/// Uses a buffer `width` columns wide. Pass a large value (e.g. 4096) for CLI
/// output so lines are never wrapped by the buffer. `buffer_to_string` strips
/// trailing spaces, so visible line length is determined by widget content.
///
/// When `no_color` is true, ANSI escape sequences are omitted entirely.
pub fn render_to_string<W: Widget>(widget: W, width: u16, height: u16, no_color: bool) -> String {
    let area = Rect::new(0, 0, width, height);
    let mut buf = Buffer::empty(area);
    widget.render(area, &mut buf);
    buffer_to_string(&buf, no_color)
}

/// Convert a rendered ratatui [`Buffer`] to a string.
///
/// Each cell's `fg` colour and `modifier` are mapped back to ANSI escape
/// sequences. When `no_color` is true, styles are ignored entirely.
///
/// Wide Unicode glyphs cause ratatui to set `cell.skip = true` on the blank
/// placeholder cell to their right. We skip those so they don't emit a space.
pub fn buffer_to_string(buf: &Buffer, no_color: bool) -> String {
    let area = buf.area();
    let mut rows: Vec<String> = Vec::with_capacity(area.height as usize);

    for y in area.top()..area.bottom() {
        let mut row = String::new();
        let mut open_escape = false;

        for x in area.left()..area.right() {
            let cell = buf.cell((x, y)).unwrap();

            // Placeholder cell behind a wide glyph — skip it.
            if cell.skip {
                continue;
            }

            let esc = if no_color {
                None
            } else {
                cell_escape(cell.fg, cell.modifier)
            };

            if open_escape {
                row.push_str("\x1b[0m");
                open_escape = false;
            }
            if let Some(ref e) = esc {
                row.push_str(e);
                open_escape = true;
            }

            row.push_str(cell.symbol());
        }

        if open_escape {
            row.push_str("\x1b[0m");
        }

        let trimmed = row.trim_end_matches(' ').to_string();
        rows.push(trimmed);
    }

    while rows.last().map(|r: &String| r.is_empty()).unwrap_or(false) {
        rows.pop();
    }

    rows.join("\n")
}

/// Map a ratatui cell's foreground colour and modifier to the ANSI escape
/// sequences used by `common::colored`. Returns `None` when no styling is
/// needed.
fn cell_escape(fg: Color, modifier: ratatui::style::Modifier) -> Option<String> {
    use ratatui::style::Modifier;

    let mut parts: Vec<&'static str> = Vec::new();

    if modifier.contains(Modifier::BOLD) {
        parts.push("\x1b[1m");
    }
    if modifier.contains(Modifier::DIM) {
        parts.push("\x1b[2m");
    }

    let color_esc: Option<&'static str> = match fg {
        Color::Red => Some("\x1b[31m"),
        Color::Green => Some("\x1b[32m"),
        Color::Yellow => Some("\x1b[33m"),
        Color::Blue => Some("\x1b[34m"),
        Color::Magenta => Some("\x1b[35m"),
        Color::Cyan => Some("\x1b[36m"),
        Color::White => Some("\x1b[37m"),
        _ => None,
    };
    if let Some(c) = color_esc {
        parts.push(c);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.concat())
    }
}
