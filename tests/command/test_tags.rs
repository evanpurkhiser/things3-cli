use crate::command::common::fixture_test;

fixture_test!(test_tags_basic_list);
fixture_test!(test_tags_empty);
fixture_test!(test_tags_filters_blank_and_whitespace_titles);
fixture_test!(test_tags_renders_shortcuts);
fixture_test!(test_tags_subtags_orphan_falls_back_to_top_level);
fixture_test!(test_tags_subtags_two_levels_deep);
fixture_test!(test_tags_subtags_under_single_parent);
