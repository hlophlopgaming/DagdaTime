use super::wayland::*;
use serde_json::json;

#[test]
fn sway_tree_finds_focused_native_and_xwayland_windows() {
    let tree = json!({"nodes":[{"nodes":[],"floating_nodes":[{"focused":true,"app_id":"editor","name":"Document","pid":42}]}]});
    let window = focused_sway(&tree).unwrap();
    assert_eq!(window.app, "editor");
    assert_eq!(window.title, "Document");
    assert_eq!(window.pid, Some(42));
    let tree =
        json!({"nodes":[{"focused":true,"window_properties":{"class":"Legacy"},"name":"Old"}]});
    assert_eq!(focused_sway(&tree).unwrap().app, "Legacy");
    assert!(focused_sway(&json!({"nodes":[]})).is_none());
}

#[test]
fn hyprland_empty_window_and_unicode_are_handled() {
    assert!(hypr_window(&json!({})).unwrap().is_none());
    let window = hypr_window(&json!({"class":"Terminal","title":"Привет","pid":99}))
        .unwrap()
        .unwrap();
    assert_eq!(window.title, "Привет");
    assert_eq!(window.pid, Some(99));
    assert!(hypr_window(&json!({"class":12})).is_err());
}

#[test]
fn bridge_requires_fresh_samples_and_preserves_null_window() {
    let state = BridgeState::default();
    assert!(state.window().is_err());
    state.update("gnome", "", "", 0, 12_000);
    assert!(state.window().unwrap().is_none());
    assert_eq!(state.idle().unwrap(), 12);
}

#[test]
fn bridge_heartbeat_keeps_window_and_updates_only_idle() {
    let state = BridgeState::default();
    assert!(!state.heartbeat(Some(0)));
    assert!(
        state.window().is_err(),
        "a heartbeat cannot invent a focused window"
    );
    state.update("gnome", "editor", "Document", 42, 12_000);
    assert!(state.heartbeat(Some(0)));
    assert_eq!(state.window().unwrap().unwrap().title, "Document");
    assert_eq!(state.idle().unwrap(), 0);
    assert!(state.heartbeat(None));
    assert_eq!(state.window().unwrap().unwrap().pid, Some(42));
}
