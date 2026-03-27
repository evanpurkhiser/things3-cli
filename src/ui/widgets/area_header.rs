use crate::common::ICONS;
use crate::ids::ThingsId;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

pub struct AreaHeaderWidget<'a> {
    pub area_uuid: &'a ThingsId,
    pub title: &'a str,
    pub id_prefix_len: usize,
}

impl<'a> AreaHeaderWidget<'a> {
    fn dim() -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    fn bold() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

impl<'a> Widget for AreaHeaderWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let id_width = self.id_prefix_len as u16;
        let [id_col, content_col] = Layout::horizontal([
            Constraint::Length(id_width + if id_width > 0 { 1 } else { 0 }),
            Constraint::Fill(1),
        ])
        .areas(area);

        if id_width > 0 {
            let id_raw: String = self
                .area_uuid
                .to_string()
                .chars()
                .take(self.id_prefix_len)
                .collect();
            Span::styled(id_raw, Self::dim()).render(
                Rect {
                    height: 1,
                    ..id_col
                },
                buf,
            );
        }

        Line::from(vec![
            Span::raw(format!("{} ", ICONS.area)),
            Span::styled(self.title, Self::bold()),
        ])
        .render(
            Rect {
                height: 1,
                ..content_col
            },
            buf,
        );
    }
}
