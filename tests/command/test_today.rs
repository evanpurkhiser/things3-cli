use crate::command::common::fixture_test;

fixture_test!(test_no_args_defaults_to_today);
fixture_test!(test_today_basic_list);
fixture_test!(test_today_detailed_with_notes_and_checklist);
fixture_test!(test_today_empty);
fixture_test!(test_today_evening_section_split);
fixture_test!(test_today_marks_staged_someday_items);
