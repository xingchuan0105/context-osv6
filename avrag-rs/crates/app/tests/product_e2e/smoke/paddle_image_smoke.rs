//! PR smoke: standalone image routes to Paddle OCR (no Docker).

use ingestion::parser::{ParseRoute, ParseRouter};

#[test]
fn png_routes_to_paddle_ocr_image_without_docker() {
    super::require_smoke_suite();
    let decision = ParseRouter::route(b"\x89PNG\r\n", "contract.png", "image/png")
        .expect("png should be routable");
    assert_eq!(decision.route, ParseRoute::PaddleOcrImage);
}
