use super::*;
use serde_json::json;

#[test]
fn crud_round_trip_and_foreign_keys() {
    let mut storage = Storage::open_in_memory().unwrap();
    let tag = storage
        .put(
            Collection::Tags,
            None,
            json!({"name":"Project","color":"#123456"}),
        )
        .unwrap();
    let id = tag["id"].as_i64().unwrap();
    let range = storage
        .put(
            Collection::TagRanges,
            None,
            json!({"tag_id":id,"start_ts":10,"end_ts":20}),
        )
        .unwrap();
    let rule = storage
        .put(
            Collection::Rules,
            None,
            json!({"tag_id":id,"field":"app","pattern":"editor"}),
        )
        .unwrap();
    assert_eq!(storage.list(Collection::Tags).unwrap(), vec![tag]);
    assert_eq!(
        storage
            .put(Collection::Tags, Some(id), json!({"name":"Renamed"}))
            .unwrap()["name"],
        "Renamed"
    );
    assert!(storage.delete(Collection::Tags, id).is_err());
    storage
        .delete(Collection::Rules, rule["id"].as_i64().unwrap())
        .unwrap();
    storage
        .delete(Collection::TagRanges, range["id"].as_i64().unwrap())
        .unwrap();
    assert!(storage.delete(Collection::Tags, id).unwrap());
    assert!(!storage.delete(Collection::Tags, id).unwrap());
}

#[test]
fn invalid_ranges_and_tag_cycles_are_rejected() {
    let mut storage = Storage::open_in_memory().unwrap();
    let parent = storage
        .put(Collection::Tags, None, json!({"name":"Parent"}))
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    let child = storage
        .put(
            Collection::Tags,
            None,
            json!({"name":"Child","parent_id":parent}),
        )
        .unwrap()["id"]
        .as_i64()
        .unwrap();
    assert!(storage
        .put(
            Collection::Tags,
            Some(parent),
            json!({"name":"Parent","parent_id":child})
        )
        .is_err());
    assert!(storage
        .put(
            Collection::TagRanges,
            None,
            json!({"tag_id":parent,"start_ts":20,"end_ts":10})
        )
        .is_err());
    assert!(storage
        .put(Collection::Tags, None, json!({"name":""}))
        .is_err());
    assert!(storage
        .put(
            Collection::Rules,
            None,
            json!({"tag_id":parent,"field":"bad","pattern":".*"})
        )
        .is_err());
}

#[test]
fn event_query_uses_half_open_ranges_and_optional_bucket() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .heartbeat("window", &json!({"app":"editor"}), 0, 10_000)
        .unwrap();
    storage
        .heartbeat("window", &json!({"app":"editor"}), 5_000, 10_000)
        .unwrap();
    assert_eq!(storage.events(None, 1_000, 2_000).unwrap().len(), 1);
    assert!(storage
        .events(Some("window"), 5_000, 6_000)
        .unwrap()
        .is_empty());
    assert!(storage.events(Some("afk"), 0, 10_000).unwrap().is_empty());
    assert!(storage.events(None, 2_000, 1_000).is_err());
}
