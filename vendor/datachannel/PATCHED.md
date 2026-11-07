# Patched datachannel 0.16.1

This is [datachannel](https://crates.io/crates/datachannel) 0.16.1 (MPL-2.0, see `LICENSE`)
with one change in `src/peerconnection.rs`. The workspace uses it through `[patch.crates-io]`.

## The change

libdatachannel calls a peer connection's Closed state callbacks (connection state and ICE
state) after moving them out of the connection, so `rtcDeletePeerConnection` does not wait for
them. When the other side closes, that callback runs on libdatachannel's thread; dropping the
`RtcPeerConnection` meanwhile freed the handler and the lock the callback was using, which
aborted with `attempt to subtract with overflow` in `lock_api`'s reentrant mutex or crashed
with SIGSEGV (specdeck #60).

The handler and its lock now live in an `Arc`. Callbacks look the connection up by id in a
process-wide registry instead of dereferencing libdatachannel's user pointer, and hold their
own reference while they run. Dropping the connection removes it from the registry, so the
handler is dropped when the last running callback returns.

Data channels do not need this: deleting one waits for its running callbacks.

`crates/core/tests/datachannel_close.rs` covers both.

## Updating

Upstream (`lerouxrgd/datachannel-rs` master and libdatachannel master) still has the original
code. A datachannel version this copy does not match leaves the patch unused (Cargo only warns),
and the connection test in `datachannel_close.rs` then fails. When upgrading, check whether
upstream fixed it; if not, copy the new release here and apply the change again.
