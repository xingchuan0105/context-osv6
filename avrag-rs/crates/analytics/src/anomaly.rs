pub fn detect_request_burst(
    timestamps_sec: &[i64],
    threshold: usize,
    window_sec: i64,
) -> Option<usize> {
    if timestamps_sec.len() < threshold {
        return None;
    }

    for start in 0..timestamps_sec.len() {
        let end = start + threshold - 1;
        if end >= timestamps_sec.len() {
            break;
        }
        if timestamps_sec[end] - timestamps_sec[start] <= window_sec {
            return Some(start);
        }
    }

    None
}
