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
fn braindump_notes_and_files_are_announced_and_bounded() {
    let mut viewer = WebViewer::new(CODE).unwrap();
    let file = viewer.add_to_braindump("crash.png", b"\x89PNG").unwrap();
    assert_eq!(js_sys_get(&file, "id").as_f64(), Some(1.0));
    assert!(viewer.add_to_braindump("", b"x").is_err());
    let note = viewer.add_note(b"Say reconnecting", None).unwrap();
    assert_eq!(js_sys_get(&note, "id").as_f64(), Some(2.0));
    let edit = viewer.add_note(b"Say reconnecting, not error", Some(4)).unwrap();
    assert_eq!(js_sys_get(&edit, "id").as_f64(), Some(3.0));
    assert!(viewer.add_note(b"", None).is_err());
    assert!(viewer.add_note(&vec![b'x'; 32 * 1024 + 1], None).is_err());
    // Uploads are not resent after a reconnect: they belong to the connection.
    assert_eq!(js_sys_get(&viewer.unanswered_requests(), "length").as_f64(), Some(0.0));
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

/// A stream frame of `kind` signed with `key`, as the host's `SessionKey` encodes one.
fn signed_frame(key: &ed25519_dalek::SigningKey, kind: u8, seq: u64, payload: &[u8]) -> Vec<u8> {
    use ed25519_dalek::Signer;
    let mut bytes = vec![kind];
    bytes.extend_from_slice(&seq.to_be_bytes());
    bytes.extend_from_slice(payload);
    let signature = key.sign(&bytes);
    bytes.extend_from_slice(&signature.to_bytes());
    bytes
}

/// A viewer welcomed by a host whose frames `key` signs.
fn welcomed(key: &ed25519_dalek::SigningKey) -> WebViewer {
    use specdeck_protocol::control::{self, HostMessage, PROTOCOL_VERSION};
    let mut viewer = WebViewer::new(CODE).unwrap();
    let welcome = control::encode(&HostMessage::Welcome {
        protocol_version: PROTOCOL_VERSION,
        viewer_id: 1,
        session_public_key: key.verifying_key().to_bytes(),
        cols: 80,
        rows: 24,
        resume_token: [7; 32],
        rejoin_room: "rejoin".into(),
        agent: specdeck_protocol::AgentId::ClaudeCode,
        timeline_supported: true,
    });
    let update = viewer.on_control(&welcome);
    assert_eq!(js_sys_get(&update, "type").as_string().as_deref(), Some("welcome"));
    viewer
}

#[wasm_bindgen_test]
fn caught_up_and_resends_are_passed_on_for_the_log() {
    const OUTPUT: u8 = 1;
    const CAUGHT_UP: u8 = 5;
    let key = ed25519_dalek::SigningKey::from_bytes(&[3; 32]);
    let mut viewer = welcomed(&key);

    let first = viewer.on_stream(&signed_frame(&key, OUTPUT, 0, b"a"));
    assert!(!js_sys_get(&first, "records").is_undefined());
    assert!(js_sys_get(&first, "caughtUp").is_undefined());
    assert!(js_sys_get(&first, "resendFrom").is_undefined());

    // Record 1 got lost: 2 is dropped and asked for from 1.
    let gap = viewer.on_stream(&signed_frame(&key, OUTPUT, 2, b"c"));
    assert!(!js_sys_get(&gap, "resend").is_undefined());
    assert_eq!(js_sys_get(&gap, "resendFrom").as_f64(), Some(1.0));

    let caught_up = viewer.on_stream(&signed_frame(&key, CAUGHT_UP, 0, b""));
    assert_eq!(js_sys_get(&caught_up, "caughtUp").as_bool(), Some(true));
    assert!(js_sys_get(&caught_up, "records").is_undefined());
}
