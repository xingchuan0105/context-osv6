#[derive(Clone, Debug, PartialEq)]
pub struct HeightState {
    pub item_id: String,
    pub predicted_px: f64,
    pub measured_px: Option<f64>,
}

impl HeightState {
    pub fn predicted(item_id: impl Into<String>, predicted_px: f64) -> Self {
        Self {
            item_id: item_id.into(),
            predicted_px,
            measured_px: None,
        }
    }

    pub fn effective_px(&self) -> f64 {
        self.measured_px.unwrap_or(self.predicted_px)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScrollMode {
    FollowBottom,
    PreserveAnchor,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VirtualAnchor {
    pub item_id: String,
    pub offset_within_item: f64,
    pub mode: ScrollMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowSlice {
    pub start_index: usize,
    pub end_index: usize,
    pub top_spacer_px: f64,
    pub bottom_spacer_px: f64,
    pub visible_ids: Vec<String>,
}

impl WindowSlice {
    pub fn pin_tail(mut self, tail_id: &str) -> Self {
        if !tail_id.is_empty() && !self.visible_ids.iter().any(|id| id == tail_id) {
            self.visible_ids.push(tail_id.to_string());
        }
        self
    }
}

pub fn compute_window(
    heights: &[HeightState],
    scroll_top: f64,
    viewport_height: f64,
    overscan: usize,
) -> WindowSlice {
    if heights.is_empty() {
        return WindowSlice {
            start_index: 0,
            end_index: 0,
            top_spacer_px: 0.0,
            bottom_spacer_px: 0.0,
            visible_ids: Vec::new(),
        };
    }

    let scroll_top = scroll_top.max(0.0);
    let viewport_height = viewport_height.max(0.0);

    let mut offset = 0.0;
    let mut start = 0;
    while start < heights.len() && offset + heights[start].effective_px() <= scroll_top {
        offset += heights[start].effective_px();
        start += 1;
    }

    let start_index = start.saturating_sub(overscan);
    let mut end_index = start;
    let mut consumed = offset;
    while end_index < heights.len() && consumed < scroll_top + viewport_height {
        consumed += heights[end_index].effective_px();
        end_index += 1;
    }
    end_index = (end_index + overscan).min(heights.len());

    let top_spacer_px = heights[..start_index]
        .iter()
        .map(HeightState::effective_px)
        .sum::<f64>();
    let rendered_height = heights[start_index..end_index]
        .iter()
        .map(HeightState::effective_px)
        .sum::<f64>();
    let total_height = heights.iter().map(HeightState::effective_px).sum::<f64>();

    WindowSlice {
        start_index,
        end_index,
        top_spacer_px,
        bottom_spacer_px: (total_height - top_spacer_px - rendered_height).max(0.0),
        visible_ids: heights[start_index..end_index]
            .iter()
            .map(|item| item.item_id.clone())
            .collect(),
    }
}

pub fn apply_measurement_delta(
    anchor: &VirtualAnchor,
    updated_item_id: &str,
    delta_px: f64,
) -> f64 {
    if anchor.mode == ScrollMode::PreserveAnchor && updated_item_id != anchor.item_id {
        delta_px
    } else {
        0.0
    }
}
