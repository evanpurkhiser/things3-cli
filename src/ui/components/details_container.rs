use iocraft::prelude::*;

/// Renders a left border rail with element content to its right.
///
/// ```text
/// │ line 0
/// │ line 1
/// └ line 2
/// ```

#[derive(Props, Default)]
pub struct DetailsContainerProps<'a> {
    pub children: Vec<AnyElement<'a>>,
}

#[component]
pub fn DetailsContainer<'a>(props: &mut DetailsContainerProps<'a>) -> impl Into<AnyElement<'a>> {
    let border = BorderCharacters {
        top_left: ' ',
        top_right: ' ',
        bottom_left: '└',
        bottom_right: ' ',
        left: '│',
        right: ' ',
        top: ' ',
        bottom: ' ',
    };

    element! {
        View(flex_direction: FlexDirection::Row, gap: 1) {
            View(
                width: 1,
                border_style: BorderStyle::Custom(border),
                border_edges: Some(Edges::Left | Edges::Bottom),
            ) {}
            View(flex_direction: FlexDirection::Column) {
                #(props.children.iter_mut())
            }
        }
    }
}
