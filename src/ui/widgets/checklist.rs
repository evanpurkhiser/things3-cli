use crate::ids::matching::shortest_unique_prefixes;
use crate::store::ChecklistItem;
use crate::ui::widgets::checklist_item::ChecklistItemWidget;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

/// Renders a list of checklist items for a single task.
pub struct ChecklistWidget<'a> {
    pub items: &'a [ChecklistItem],
    /// Whether to show IDs. When true, unique prefix strings are computed and
    /// passed to each child widget.
    pub show_ids: bool,
}

impl<'a> ChecklistWidget<'a> {
    /// Width of the id column that will be rendered for these checklist items
    /// (id chars + 1 separator space), or 0 when `show_ids` is false.
    ///
    /// This lets callers (e.g. `task_item`) compute how far to offset the
    /// checklist area so that checklist IDs align with the task ID above when
    /// the two sets have different unique-prefix lengths.
    pub fn id_col_width(items: &[ChecklistItem], show_ids: bool) -> u16 {
        if !show_ids || items.is_empty() {
            return 0;
        }
        let ids: Vec<_> = items.iter().map(|i| i.uuid.clone()).collect();
        let prefixes = shortest_unique_prefixes(&ids);
        // All prefixes have the same length (they're computed over the same
        // set).  Take the first one as the canonical width.
        let prefix_len = prefixes.values().next().map(|s| s.len()).unwrap_or(1);
        (prefix_len + 1) as u16 // id chars + separator space
    }
}

impl<'a> Widget for ChecklistWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.items.is_empty() || area.height == 0 {
            return;
        }

        let id_col_width = Self::id_col_width(self.items, self.show_ids);
        let prefixes = if id_col_width > 0 {
            Some(shortest_unique_prefixes(
                &self
                    .items
                    .iter()
                    .map(|i| i.uuid.clone())
                    .collect::<Vec<_>>(),
            ))
        } else {
            None
        };

        for (i, item) in self.items.iter().enumerate() {
            let row = Rect {
                y: area.y + i as u16,
                height: 1,
                ..area
            };
            ChecklistItemWidget {
                item,
                id: prefixes
                    .as_ref()
                    .and_then(|p| p.get(&item.uuid).map(|s| s.as_str())),
                is_last: i == self.items.len() - 1,
            }
            .render(row, buf);
        }
    }
}
