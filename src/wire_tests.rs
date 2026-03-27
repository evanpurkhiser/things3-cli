#[cfg(test)]
mod tests {
    use crate::ids::ThingsId;
    use crate::wire::checklist::ChecklistItemProps;
    use crate::wire::recurrence::{FrequencyUnit, RecurrenceRule};
    use crate::wire::tags::TagPatch;
    use crate::wire::task::{TaskPatch, TaskProps, TaskStart, TaskStatus};
    use crate::wire::wire_object::WireItem;
    use crate::wire::wire_object::{EntityType, OperationType, Properties, WireObject};

    fn id(s: &str) -> ThingsId {
        ThingsId::from(s)
    }

    const ID_A: &str = "A7h5eCi24RvAWKC3Hv3muf";
    const ID_B: &str = "MpkEei6ybkFS2n6SXvwfLf";
    const ID_C: &str = "JFdhhhp37fpryAKu8UXwzK";

    #[test]
    fn wire_object_deserializes_with_wire_keys() {
        let json = r#"{
            "abc-123": {
                "t": 1,
                "e": "Task6",
                "p": {"tt": "Title", "ss": 0}
            }
        }"#;

        let item: WireItem = serde_json::from_str(json).expect("valid wire item");
        let object = item.get("abc-123").expect("object exists");

        assert_eq!(object.operation_type, OperationType::Update);
        assert_eq!(object.entity_type, Some(EntityType::Task6));
        assert_eq!(
            object
                .properties_map()
                .get("tt")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .as_deref(),
            Some("Title")
        );
    }

    #[test]
    fn task_props_maps_readable_fields_to_wire_names() {
        let props = TaskProps {
            title: "Ship v1".to_string(),
            status: TaskStatus::Completed,
            start_location: TaskStart::Anytime,
            parent_project_ids: vec![id(ID_A)],
            area_ids: vec![id(ID_B)],
            tag_ids: vec![id(ID_C)],
            evening_bit: 1,
            ..TaskProps::default()
        };

        let encoded = serde_json::to_value(props).expect("serialize task props");

        assert_eq!(encoded.get("tt").and_then(|v| v.as_str()), Some("Ship v1"));
        assert_eq!(encoded.get("ss").and_then(|v| v.as_i64()), Some(3));
        assert_eq!(encoded.get("st").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(encoded.get("sb").and_then(|v| v.as_i64()), Some(1));
        assert!(encoded.get("title").is_none());
        assert!(encoded.get("status").is_none());
    }

    #[test]
    fn task_props_accepts_null_for_defaulted_scalar_fields() {
        let json = r#"{
            "tt": "Personal Website",
            "tp": 1,
            "ss": 0,
            "st": 1,
            "pr": [],
            "ar": [],
            "agr": [],
            "tg": [],
            "ix": -7069,
            "ti": null,
            "do": null,
            "rt": [],
            "icc": null,
            "icp": true,
            "sb": null,
            "lt": null,
            "tr": false,
            "dl": []
        }"#;

        let parsed: TaskProps = serde_json::from_str(json).expect("valid task props with nulls");
        assert_eq!(parsed.today_sort_index, 0);
        assert_eq!(parsed.due_date_offset, 0);
        assert_eq!(parsed.checklist_item_count, 0);
        assert_eq!(parsed.evening_bit, 0);
        assert!(!parsed.leaves_tombstone);
    }

    #[test]
    fn checklist_item_accepts_task_ids_list_wire_shape() {
        let json = r#"{
            "tt": "One",
            "ss": 0,
            "ts": ["A7h5eCi24RvAWKC3Hv3muf"],
            "ix": 9
        }"#;

        let parsed: ChecklistItemProps =
            serde_json::from_str(json).expect("valid checklist item props");
        assert_eq!(parsed.title, "One");
        assert_eq!(parsed.task_ids, vec![id(ID_A)]);
        assert_eq!(parsed.sort_index, 9);
    }

    #[test]
    fn checklist_item_accepts_single_task_id_wire_shape() {
        let json = r#"{
            "tt": "One",
            "ss": 0,
            "ts": "A7h5eCi24RvAWKC3Hv3muf",
            "ix": 9
        }"#;

        let parsed: ChecklistItemProps =
            serde_json::from_str(json).expect("valid checklist item props");
        assert_eq!(parsed.title, "One");
        assert_eq!(parsed.task_ids, vec![id(ID_A)]);
        assert_eq!(parsed.sort_index, 9);
    }

    #[test]
    fn checklist_item_accepts_null_lt_as_default_false() {
        let json = r#"{
            "tt": "One",
            "ss": 0,
            "ts": ["A7h5eCi24RvAWKC3Hv3muf"],
            "ix": 9,
            "lt": null
        }"#;

        let parsed: ChecklistItemProps =
            serde_json::from_str(json).expect("valid checklist item props with null lt");
        assert!(!parsed.leaves_tombstone);
    }

    #[test]
    fn checklist_item_create_omits_unset_optional_fields() {
        let props = ChecklistItemProps {
            title: "One".to_string(),
            status: TaskStatus::Incomplete,
            task_ids: vec![id(ID_A)],
            sort_index: 9,
            creation_date: Some(1.0),
            modification_date: Some(2.0),
            ..ChecklistItemProps::default()
        };

        let encoded = serde_json::to_value(props).expect("serialize checklist props");

        assert_eq!(encoded.get("tt").and_then(|v| v.as_str()), Some("One"));
        assert_eq!(encoded.get("ss").and_then(|v| v.as_i64()), Some(0));
        assert_eq!(encoded.get("ix").and_then(|v| v.as_i64()), Some(9));
        assert!(encoded.get("sp").is_none());
        assert!(encoded.get("lt").is_none());
        assert!(encoded.get("xx").is_none());
    }

    #[test]
    fn recurrence_rule_defaults_match_protocol() {
        let parsed: RecurrenceRule = serde_json::from_str("{}")
            .expect("empty recurrence should deserialize with protocol defaults");

        assert_eq!(parsed.frequency_unit, FrequencyUnit::Weekly);
        assert_eq!(parsed.frequency_amount, 1);
        assert_eq!(parsed.end_date, 64_092_211_200);
        assert_eq!(parsed.recurrence_rule_version, 4);
    }

    #[test]
    fn operation_enum_serializes_to_wire_integer() {
        let object = WireObject {
            operation_type: OperationType::Delete,
            entity_type: None,
            payload: Properties::Delete,
        };
        let json = serde_json::to_string(&object).expect("serialize wire object");
        assert!(json.contains("\"t\":2"));
    }

    #[test]
    fn unknown_numeric_enum_values_round_trip() {
        let parsed: WireObject = serde_json::from_str(r#"{"t":99,"e":"Task6","p":{}}"#)
            .expect("deserialize with unknown op type");

        assert_eq!(parsed.operation_type, OperationType::Unknown(99));

        let json = serde_json::to_string(&parsed).expect("serialize unknown op type");
        assert!(json.contains("\"t\":99"));
    }

    #[test]
    fn unknown_entity_values_round_trip() {
        let parsed: WireObject =
            serde_json::from_str(r#"{"t":1,"e":"Task7","p":{}}"#).expect("deserialize");
        assert_eq!(
            parsed.entity_type,
            Some(EntityType::Unknown("Task7".to_string()))
        );

        let json = serde_json::to_string(&parsed).expect("serialize unknown entity");
        assert!(json.contains("\"e\":\"Task7\""));
    }

    #[test]
    fn typed_properties_dispatch_for_task_create() {
        let parsed: WireObject =
            serde_json::from_str(r#"{"t":0,"e":"Task6","p":{"tt":"A","ss":0,"tp":0,"st":0}}"#)
                .expect("deserialize");

        let typed = parsed.properties().expect("typed properties");
        match typed {
            Properties::TaskCreate(props) => {
                assert_eq!(props.title, "A");
                assert_eq!(props.status, TaskStatus::Incomplete);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn typed_properties_dispatch_for_delete() {
        let parsed: WireObject =
            serde_json::from_str(r#"{"t":2,"e":"Task6","p":{}}"#).expect("deserialize");
        let typed = parsed.properties().expect("typed properties");
        assert!(matches!(typed, Properties::Delete));
    }

    #[test]
    fn task_patch_preserves_explicit_null_for_clearable_fields() {
        let patch: TaskPatch = serde_json::from_str(r#"{"sr":null,"tir":null,"sp":null}"#)
            .expect("deserialize patch with nulls");

        assert_eq!(patch.scheduled_date, Some(None));
        assert_eq!(patch.today_index_reference, Some(None));
        assert_eq!(patch.stop_date, Some(None));
    }

    #[test]
    fn task_update_wire_object_keeps_null_clears() {
        let parsed: WireObject =
            serde_json::from_str(r#"{"t":1,"e":"Task6","p":{"sr":null,"tir":null}}"#)
                .expect("deserialize wire update");

        let typed = parsed.properties().expect("typed properties");
        match typed {
            Properties::TaskUpdate(patch) => {
                assert_eq!(patch.scheduled_date, Some(None));
                assert_eq!(patch.today_index_reference, Some(None));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn tag_patch_preserves_explicit_null_for_shortcut() {
        let patch: TagPatch =
            serde_json::from_str(r#"{"sh":null}"#).expect("deserialize tag patch with null");
        assert_eq!(patch.shortcut, Some(None));
    }

    #[test]
    fn tag_update_wire_object_keeps_null_shortcut_clear() {
        let parsed: WireObject = serde_json::from_str(r#"{"t":1,"e":"Tag4","p":{"sh":null}}"#)
            .expect("deserialize tag wire update");

        let typed = parsed.properties().expect("typed properties");
        match typed {
            Properties::TagUpdate(patch) => {
                assert_eq!(patch.shortcut, Some(None));
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
