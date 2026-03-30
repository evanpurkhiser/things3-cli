pub mod components;
pub mod utils;
pub mod views;

use iocraft::prelude::*;

pub fn render_lines(lines: Vec<String>, no_color: bool) -> String {
    let mut element = element! {
        View(flex_direction: FlexDirection::Column) {
            #(lines.into_iter().map(|line| element! { Text(content: line, wrap: TextWrap::NoWrap) }))
        }
    };

    render_element_to_string(&mut element, no_color)
}

pub fn indent_lines(lines: Vec<String>, spaces: usize) -> Vec<String> {
    let prefix = " ".repeat(spaces);
    lines
        .into_iter()
        .map(|line| {
            if line.is_empty() {
                line
            } else {
                format!("{}{}", prefix, line)
            }
        })
        .collect()
}

pub fn render_element_to_string<E: ElementExt>(element: &mut E, no_color: bool) -> String {
    let canvas = element.render(None);
    let mut bytes = Vec::new();
    if no_color {
        canvas
            .write(&mut bytes)
            .expect("writing iocraft canvas should not fail");
    } else {
        canvas
            .write_ansi(&mut bytes)
            .expect("writing ANSI iocraft canvas should not fail");
    }

    let s = String::from_utf8(bytes).expect("iocraft output should be UTF-8");
    let s = s.replace("\u{1b}[K", "");
    let mut lines = s
        .split('\n')
        .map(|line| line.trim_end_matches(' ').to_string())
        .collect::<Vec<_>>();

    while lines.last().map(|line| line.is_empty()).unwrap_or(false) {
        lines.pop();
    }

    lines.join("\n")
}
