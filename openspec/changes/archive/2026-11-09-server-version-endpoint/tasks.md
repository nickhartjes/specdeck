# Tasks

## 1. Service

- [x] 1.1 In `crates/server/src/web.rs`, add `"version"` to the `/web-viewer.json` body from `env!("CARGO_PKG_VERSION")` (the workspace version, which the release workflow checks against the tag), keeping `protocol` and the existing headers
- [x] 1.2 Update `the_app_can_tell_the_page_is_there_and_its_protocol` in `crates/server/tests/web.rs` to expect `{"protocol":N,"version":"<CARGO_PKG_VERSION>"}`, and assert `/health` still answers `ok`
- [x] 1.3 Check that the app's reader in `app/src-tauri/src/services.rs` still parses a body with the extra field (it reads `protocol` from a JSON value); add a test case for it there

## 2. Docs

- [x] 2.1 In `docs/site/src/content/docs/self-host/docker-compose.md`, end the Update section with checking the running version: `curl https://<service>/web-viewer.json`
- [x] 2.2 Add the same check to the upgrade steps in `docs/site/src/content/docs/self-host/kubernetes.md`

## 3. Verify

- [x] 3.1 `cargo test -p specdeck-server --features web-viewer` and the app crate's tests pass
- [x] 3.2 Run the server locally with the web viewer and confirm `curl localhost:8080/web-viewer.json` shows the workspace version
