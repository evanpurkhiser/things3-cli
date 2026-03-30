use crate::ids::ThingsId;
use crate::ui::utils::id_prefix;
use iocraft::prelude::*;

#[derive(Default, Props)]
pub struct HeaderProps<'a> {
    pub uuid: Option<&'a ThingsId>,
    pub title: Option<&'a str>,
    pub id_prefix_len: usize,
    pub icon: Option<&'a str>,
}

#[component]
pub fn Header<'a>(props: &HeaderProps<'a>) -> impl Into<AnyElement<'a>> {
    let (Some(uuid), Some(title)) = (props.uuid, props.title) else {
        return element!(Fragment).into_any();
    };

    let id = id_prefix(uuid, props.id_prefix_len);

    let line = if let Some(icon) = props.icon {
        format!("{} {} {}", id, icon, title)
    } else {
        format!("{} {}", id, title)
    };

    element! { Text(content: line, wrap: TextWrap::NoWrap) }.into_any()
}
