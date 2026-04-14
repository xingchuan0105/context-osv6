use web_ui::state::virtual_list::{
    HeightState, ScrollMode, VirtualAnchor, apply_measurement_delta, compute_window,
};

#[test]
fn compute_window_includes_visible_rows_and_overscan() {
    let heights = vec![
        HeightState::predicted("a", 100.0),
        HeightState::predicted("b", 100.0),
        HeightState::predicted("c", 100.0),
        HeightState::predicted("d", 100.0),
    ];

    let window = compute_window(&heights, 120.0, 160.0, 1);

    assert_eq!(window.start_index, 0);
    assert_eq!(window.end_index, 4);
    assert_eq!(window.top_spacer_px, 0.0);
    assert_eq!(window.visible_ids, vec!["a", "b", "c", "d"]);
}

#[test]
fn anchor_compensation_tracks_delta_above_anchor() {
    let anchor = VirtualAnchor {
        item_id: "c".to_string(),
        offset_within_item: 18.0,
        mode: ScrollMode::PreserveAnchor,
    };

    let delta = apply_measurement_delta(&anchor, "b", 40.0);
    assert_eq!(delta, 40.0);
}

#[test]
fn pinned_tail_never_drops_last_row() {
    let heights = vec![
        HeightState::predicted("a", 120.0),
        HeightState::predicted("b", 120.0),
        HeightState::predicted("tail", 240.0),
    ];

    let window = compute_window(&heights, 0.0, 120.0, 0).pin_tail("tail");
    assert!(window.visible_ids.iter().any(|id| id == "tail"));
}
