use crate::common::ICONS;
use crate::store::ChecklistItem;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Span,
    widgets::Widget,
};

/// A single checklist-item row.
///
/// When `id` is `Some`, it is rendered in a left column sized to fit it, so
/// that checklist IDs align with the task ID above. The connector (`├╴`/`└╴`)
/// and icon overwrite the `│`/`└` drawn earlier by [`LeftBorderWidget`].
///
/// ```text
/// M ├╴○ Confirm changelog
/// J └╴● Tag release commit   (is_last)
/// ```
///
/// When `id` is `None` (no IDs), the connector starts at column 0:
/// ```text
/// ├╴○ title
/// └╴● title
/// ```
pub struct ChecklistItemWidget<'a> {
    pub item: &'a ChecklistItem,
    /// The shortest unique prefix string for this item's UUID, or `None` when
    /// IDs are not being shown. All ids in the group have the same length.
    pub id: Option<&'a str>,
    /// Whether this is the last item (selects `└╴` vs `├╴`).
    pub is_last: bool,
}

impl<'a> ChecklistItemWidget<'a> {
    fn icon_str(&self) -> &'static str {
        if self.item.is_completed() {
            ICONS.checklist_done
        } else if self.item.is_canceled() {
            ICONS.checklist_canceled
        } else {
            ICONS.checklist_open
        }
    }
}

impl<'a> Widget for ChecklistItemWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let dim = Style::default().add_modifier(Modifier::DIM);
        let row = Rect { height: 1, ..area };

        let connector = if self.is_last { "└╴" } else { "├╴" };

        if let Some(id_str) = self.id {
            // Layout: [id(len)][space(1)][connector(2)][icon(1)][space(1)][title(Fill)]
            let id_len = id_str.len() as u16;
            let chunks = Layout::horizontal([
                Constraint::Length(id_len), // id prefix
                Constraint::Length(1),      // separator space
                Constraint::Length(2),      // ├╴ or └╴
                Constraint::Length(1),      // ○/●/×
                Constraint::Length(1),      // space
                Constraint::Fill(1),        // title
            ])
            .split(row);

            Span::styled(id_str, dim).render(chunks[0], buf);
            // chunks[1] = separator space, left blank
            Span::styled(connector, dim).render(chunks[2], buf);
            Span::styled(self.icon_str(), dim).render(chunks[3], buf);
            // chunks[4] = space, left blank
            Span::raw(self.item.title.clone()).render(chunks[5], buf);
        } else {
            // No IDs — connector starts at x=0 of area.
            let chunks = Layout::horizontal([
                Constraint::Length(2), // ├╴ or └╴
                Constraint::Length(1), // ○/●/×
                Constraint::Length(1), // space
                Constraint::Fill(1),   // title
            ])
            .split(row);

            Span::styled(connector, dim).render(chunks[0], buf);
            Span::styled(self.icon_str(), dim).render(chunks[1], buf);
            // chunks[2] = space, left blank
            Span::raw(self.item.title.clone()).render(chunks[3], buf);
        }
    }
}
