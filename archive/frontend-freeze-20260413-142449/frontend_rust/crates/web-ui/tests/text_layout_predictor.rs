use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use web_ui::platform::text_layout::{TypographyProfile, estimate_shell_height, width_bucket};

fn profile() -> TypographyProfile {
    TypographyProfile {
        font_css: "16px Inter".to_string(),
        line_height_px: 24.0,
        horizontal_padding_px: 32.0,
        vertical_padding_px: 24.0,
        block_gap_px: 12.0,
        reserved_width_px: 40.0,
    }
}

#[test]
fn width_bucket_rounds_down_to_32px_steps() {
    assert_eq!(width_bucket(641.0), 640);
    assert_eq!(width_bucket(767.0), 736);
}

#[test]
fn shell_height_adds_padding_and_block_gap() {
    let profile = profile();
    let height = estimate_shell_height(96.0, &profile, 2);
    assert_eq!(height, 96.0 + 24.0 + 24.0 + 12.0);
}

#[test]
fn empty_text_predicts_zero_height_and_zero_lines() {
    let profile = profile();
    let prediction = block_on(web_ui::platform::text_layout::predict_text_height(
        "", &profile, "en", 320.0,
    ))
    .unwrap();

    assert_eq!(prediction.text_height_px, 0.0);
    assert_eq!(prediction.line_count, 0);
}

#[test]
fn explicit_newlines_are_preserved_in_fallback_height() {
    let profile = profile();
    let prediction = block_on(web_ui::platform::text_layout::predict_text_height(
        "first line\nsecond line\nthird line",
        &profile,
        "en",
        320.0,
    ))
    .unwrap();

    assert_eq!(prediction.line_count, 3);
    assert_eq!(prediction.text_height_px, 72.0);
}

fn block_on<F: Future>(mut future: F) -> F::Output {
    fn raw_waker() -> RawWaker {
        fn no_op(_: *const ()) {}
        fn clone(_: *const ()) -> RawWaker {
            raw_waker()
        }
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }

    let waker = unsafe { Waker::from_raw(raw_waker()) };
    let mut context = Context::from_waker(&waker);
    let mut future = unsafe { Pin::new_unchecked(&mut future) };
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => continue,
        }
    }
}
