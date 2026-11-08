//! The WebAssembly build, run in Node: `just web-test`.
#![cfg(target_arch = "wasm32")]

use specdeck_protocol::files::{self, FilesReply};
use specdeck_protocol::invite::InviteCode;
use specdeck_protocol::signal_auth::{DescriptionKind, SignedDescription};
use specdeck_web::{FileFetch, FileUpload, WebViewer, parse_join_link};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test;

const CODE: &str = "7K2M-9QXH-4TPA";

/// The host's answer, signed with `code`'s key for `connection`.
fn answer(code: &str, connection: &str) -> JsValue {
    let key = InviteCode::parse(code).unwrap().keys().auth_key;
    let signed =
        SignedDescription::sign(&key, connection, DescriptionKind::Answer, "v=0 answer".into());
    serde_wasm_bindgen::to_value(&signed).unwrap()
}

#[wasm_bindgen_test]
fn joins_the_codes_room_and_signs_its_offer_with_the_codes_key() {
    let mut viewer = WebViewer::new("7k2m9qxh4tpa").unwrap();
    let code = InviteCode::parse(CODE).unwrap();
    assert_eq!(viewer.start_attempt(), code.keys().room_id);

    let offer: SignedDescription =
        serde_wasm_bindgen::from_value(viewer.sign_offer("c1", "v=0 offer".into())).unwrap();
    assert_eq!(offer.verify(&code.keys().auth_key, "c1", DescriptionKind::Offer), Ok("v=0 offer"));
}

#[wasm_bindgen_test]
fn accepts_only_the_answer_made_with_the_sessions_code_for_this_connection() {
    let mut viewer = WebViewer::new(CODE).unwrap();
    viewer.start_attempt();
    assert_eq!(viewer.verify_answer("c1", answer(CODE, "c1")), Some("v=0 answer".into()));
    // Made with another code, as the signalling service would have to.
    assert_eq!(viewer.verify_answer("c1", answer("AAAA-BBBB-CCCC", "c1")), None);
    // Meant for another connection.
    assert_eq!(viewer.verify_answer("c2", answer(CODE, "c1")), None);
    assert_eq!(viewer.verify_answer("c1", JsValue::from_str("garbage")), None);
}

#[wasm_bindgen_test]
fn suggestions_are_numbered_and_checked_before_sending() {
    let mut viewer = WebViewer::new(CODE).unwrap();
    let first = viewer.suggest("Add a test\r\nfor the heartbeat").unwrap();
    assert_eq!(js_sys_get(&first, "id").as_f64(), Some(1.0));
    let second = viewer.suggest("Fix it").unwrap();
    assert_eq!(js_sys_get(&second, "id").as_f64(), Some(2.0));
    // Unanswered until the host replies, so they go out once admitted.
    let unanswered = viewer.unanswered_requests();
    assert_eq!(js_sys_get(&unanswered, "length").as_f64(), Some(2.0));
    assert!(viewer.suggest("   ").is_err());
    assert!(viewer.suggest(&"x".repeat(4097)).is_err());
    assert!(viewer.suggest("run\u{1b}[2J").is_err());

    let reply =
        specdeck_protocol::control::encode(&specdeck_protocol::control::HostMessage::Reply {
            id: 1,
            outcome: specdeck_protocol::control::Outcome::Received,
        });
    let update = viewer.on_control(&reply);
    assert_eq!(js_sys_get(&update, "type").as_string().as_deref(), Some("reply"));
    assert_eq!(js_sys_get(&update, "outcome").as_string().as_deref(), Some("Received"));
    let unanswered = viewer.unanswered_requests();
    assert_eq!(js_sys_get(&unanswered, "length").as_f64(), Some(1.0));
}

#[wasm_bindgen_test]
fn a_fetched_file_is_checked_and_fetched_again_once_when_altered() {
    let content = b"# Notes";
    let mut fetch = FileFetch::new(3, content.len() as u64, &files::sha256(content)).unwrap();
    assert!(!fetch.request().is_empty());
    let chunk = |bytes: &[u8]| {
        files::encode_reply(&FilesReply::Chunk { file: 3, offset: 0, bytes: bytes.to_vec() })
    };
    let retry = fetch.on_reply(&chunk(b"# Notez"));
    assert!(!js_sys_get(&retry, "retry").is_undefined(), "fetched again after a bad hash");
    let done = fetch.on_reply(&chunk(content));
    assert!(!js_sys_get(&done, "done").is_undefined(), "arrived whole");
    let other = fetch.on_reply(&files::encode_reply(&FilesReply::Gone { file: 4 }));
    assert!(js_sys_get(&other, "failed").is_undefined(), "about another file");
}

#[wasm_bindgen_test]
fn a_file_to_share_is_announced_with_its_hash_and_sent_after_ready() {
    let mut viewer = WebViewer::new(CODE).unwrap();
    let request = viewer.share_file("notes.md", b"# Notes").unwrap();
    assert_eq!(js_sys_get(&request, "id").as_f64(), Some(1.0));
    assert!(viewer.share_file("", b"x").is_err());
    let mut upload = FileUpload::new(5, b"# Notes");
    assert_eq!(js_sys_get(&upload.next_chunks(), "length").as_f64(), Some(1.0));
    let finished = files::encode_reply(&FilesReply::Finished {
        upload: 5,
        outcome: specdeck_protocol::control::Outcome::Received,
    });
    let update = upload.on_reply(&finished);
    assert_eq!(js_sys_get(&update, "finished").as_string().as_deref(), Some("Received"));
}

#[wasm_bindgen_test]
fn refuses_what_is_not_a_code() {
    assert!(WebViewer::new("not a code").is_err());
}

#[wasm_bindgen_test]
fn a_garbled_control_message_means_retry() {
    let mut viewer = WebViewer::new(CODE).unwrap();
    let update = viewer.on_control(b"garbage");
    let kind = js_sys_get(&update, "type");
    assert_eq!(kind.as_string().as_deref(), Some("retry"));
}

#[wasm_bindgen_test]
fn reads_both_kinds_of_join_link() {
    let web = parse_join_link("https://specdeck.example.com/join#7k2m9qxh4tpa");
    assert_eq!(js_sys_get(&web, "code").as_string().as_deref(), Some(CODE));
    assert_eq!(
        js_sys_get(&web, "service").as_string().as_deref(),
        Some("wss://specdeck.example.com/signal")
    );
    let app = parse_join_link("specdeck://join/7K2M-9QXH-4TPA");
    assert_eq!(js_sys_get(&app, "code").as_string().as_deref(), Some(CODE));
    assert!(js_sys_get(&app, "service").is_undefined());
    assert!(parse_join_link("https://example.com/join?code=7K2M-9QXH-4TPA").is_undefined());
}

/// `object[key]`, without pulling in js-sys for one call.
fn js_sys_get(object: &JsValue, key: &str) -> JsValue {
    #[wasm_bindgen::prelude::wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = Reflect, js_name = get)]
        fn reflect_get(target: &JsValue, key: &JsValue) -> JsValue;
    }
    reflect_get(object, &JsValue::from_str(key))
}
