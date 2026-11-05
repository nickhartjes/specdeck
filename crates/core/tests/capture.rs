//! The test capture itself: it must see every event emitted in its scope, whatever other
//! threads did first.

#[path = "support/capture.rs"]
mod capture;

use capture::Capture;

fn shared_event() {
    tracing::info!("capture.shared");
}

#[test]
fn an_event_first_emitted_outside_any_capture_is_still_captured() {
    let (capture, _guard) = Capture::start();
    // Another test's thread, with no capture of its own, is the first to reach this event.
    std::thread::spawn(shared_event).join().unwrap();

    shared_event();
    assert_eq!(capture.named("capture.shared").len(), 1);
}
