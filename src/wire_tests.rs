#[cfg(test)]
mod tests {
    use crate::wire::{
        ChecklistItemProps, EntityType, FrequencyUnit, OperationType, RecurrenceRule, TaskProps,
        TaskStart, TaskStatus, WireItem, WireObject,
    };

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
            object.properties.get("tt").and_then(|v| v.as_str()),
            Some("Title")
        );
    }

    #[test]
    fn task_props_maps_readable_fields_to_wire_names() {
        let props = TaskProps {
            title: "Ship v1".to_string(),
            status: TaskStatus::Completed,
            start_location: TaskStart::Anytime,
            parent_project_ids: vec!["project-1".to_string()],
            area_ids: vec!["area-1".to_string()],
            tag_ids: vec!["tag-1".to_string()],
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
    fn checklist_item_accepts_task_ids_list_wire_shape() {
        let json = r#"{
            "tt": "One",
            "ss": 0,
            "ts": ["task-123"],
            "ix": 9
        }"#;

        let parsed: ChecklistItemProps =
            serde_json::from_str(json).expect("valid checklist item props");
        assert_eq!(parsed.title, "One");
        assert_eq!(parsed.task_ids, vec!["task-123"]);
        assert_eq!(parsed.sort_index, 9);
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
            properties: Default::default(),
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
}
