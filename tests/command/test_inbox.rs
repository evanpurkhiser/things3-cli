use crate::command::common::fixture_test;

fixture_test!(test_inbox_basic_list);
fixture_test!(test_inbox_detailed_mode_shows_notes_and_checklist);
fixture_test!(test_inbox_empty);
fixture_test!(test_inbox_ignores_project_and_area_scoped_tasks);
