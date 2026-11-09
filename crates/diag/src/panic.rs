//! Crashes in the log (the `diagnostics` spec, *Crashes in diagnostics*): a panic hook that
//! records where and what kind of panic happened, never its message, which can hold paths or
//! the user's text. The line goes straight to the file (see `Delivery::Now`), so it is there
//! even when the process ends next.

use std::panic::PanicHookInfo;

/// Our own crates; a panic in one of them keeps its file and line.
const OWN_CRATES: [&str; 7] = ["core", "diag", "local", "protocol", "server", "session", "web"];

/// Records `event` (`app.panicked` or `session.panicked`) on every panic, then lets the panic
/// go on as before.
pub fn install_panic_hook(event: &'static str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        record(event, info);
        previous(info);
    }));
}

fn record(event: &'static str, info: &PanicHookInfo<'_>) {
    let thread = std::thread::current();
    let location = info.location().map_or("unknown".to_owned(), |l| location(l.file(), l.line()));
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
        .unwrap_or_default();
    tracing::error!(
        target: "specdeck_diag",
        thread = thread.name().unwrap_or("unnamed"),
        location,
        kind = kind(message),
        "{event}"
    );
}

/// Where a panic happened, without anything of the machine it happened on: `crates/…:line`
/// in our own code, `crate:<name>` in a dependency, `std` in Rust's own library, else `other`.
pub fn location(file: &str, line: u32) -> String {
    let file = file.replace('\\', "/");
    let after = |marker: &str| file.find(marker).map(|at| &file[at + marker.len()..]);
    let first = |rest: &str| rest.split('/').next().unwrap_or_default().to_owned();
    if file.starts_with("/rustc/") || file.contains("/rustlib/src/") {
        return "std".into();
    }
    // `…/registry/src/<index>/<crate>-<version>/…`
    if let Some(rest) = after("/registry/src/") {
        let name = rest.split('/').nth(1).unwrap_or_default();
        return if name.is_empty() { "other".into() } else { format!("crate:{name}") };
    }
    // `…/git/checkouts/<repo>-<hash>/…`
    if let Some(rest) = after("/git/checkouts/") {
        return format!("crate:{}", first(rest));
    }
    let own = |at: usize| {
        let rest = &file[at..];
        let ours = match rest.strip_prefix("crates/") {
            Some(crate_path) => OWN_CRATES.iter().any(|name| {
                crate_path.strip_prefix(name).is_some_and(|tail| tail.starts_with("/src/"))
            }),
            None => rest.starts_with("app/src-tauri/src/"),
        };
        ours.then(|| format!("{rest}:{line}"))
    };
    let starts = std::iter::once(0)
        .chain(file.match_indices("/crates/").map(|(at, _)| at + 1))
        .chain(file.match_indices("/app/src-tauri/").map(|(at, _)| at + 1));
    starts.filter_map(own).last().unwrap_or_else(|| "other".into())
}

/// The kind of panic, from the start of the message std itself writes; any other message,
/// such as an `expect`'s or a `panic!`'s own text, is `other`.
pub fn kind(message: &str) -> &'static str {
    let starts = |prefixes: &[&str]| prefixes.iter().any(|prefix| message.starts_with(prefix));
    if message.contains("PoisonError") {
        "poisoned"
    } else if starts(&["called `Option::unwrap()` on a `None` value"]) {
        "unwrap_none"
    } else if starts(&["called `Result::unwrap()` on an `Err` value"]) {
        "unwrap_err"
    } else if starts(&[
        "index out of bounds",
        "range end index",
        "range start index",
        "byte index",
        "slice index starts",
    ]) {
        "index_out_of_bounds"
    } else if message.starts_with("attempt to ") && message.ends_with(" with overflow") {
        "overflow"
    } else if starts(&["already borrowed", "already mutably borrowed", "RefCell already"]) {
        "borrow"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_keep_nothing_of_the_machine() {
        let cases = [
            ("crates/core/src/session.rs", "crates/core/src/session.rs:7"),
            ("app/src-tauri/src/lib.rs", "app/src-tauri/src/lib.rs:7"),
            ("/Users/marker-user/code/specdeck/crates/core/src/x.rs", "crates/core/src/x.rs:7"),
            (
                "C:\\Users\\marker-user\\specdeck\\crates\\diag\\src\\lib.rs",
                "crates/diag/src/lib.rs:7",
            ),
            ("/build/source/crates/session/src/main.rs", "crates/session/src/main.rs:7"),
            ("/build/source/app/src-tauri/src/updates.rs", "app/src-tauri/src/updates.rs:7"),
            (
                "/Users/marker-user/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tokio-1.47.1/src/runtime/park.rs",
                "crate:tokio-1.47.1",
            ),
            (
                "/home/marker-user/.cargo/git/checkouts/datachannel-rs-1a2b3c/4d5e6f7/src/lib.rs",
                "crate:datachannel-rs-1a2b3c",
            ),
            ("/rustc/58cf8f9c3/library/core/src/option.rs", "std"),
            ("/Users/marker-user/notes/crates/secret/src/x.rs", "other"),
            ("/Users/marker-user/elsewhere/main.rs", "other"),
        ];
        for (file, expected) in cases {
            let location = location(file, 7);
            assert_eq!(location, expected, "{file}");
            assert!(!location.contains("marker"), "{location}");
        }
    }

    #[test]
    fn kinds_from_std_messages_only() {
        let cases = [
            ("called `Option::unwrap()` on a `None` value", "unwrap_none"),
            ("called `Result::unwrap()` on an `Err` value: Os { code: 2 }", "unwrap_err"),
            ("called `Result::unwrap()` on an `Err` value: PoisonError { .. }", "poisoned"),
            ("index out of bounds: the len is 3 but the index is 5", "index_out_of_bounds"),
            ("range end index 9 out of range for slice of length 4", "index_out_of_bounds"),
            ("byte index 3 is not a char boundary", "index_out_of_bounds"),
            ("attempt to subtract with overflow", "overflow"),
            ("already borrowed: BorrowMutError", "borrow"),
            ("Marker-Expect-Text: /Users/marker-user/x", "other"),
            ("", "other"),
        ];
        for (message, expected) in cases {
            assert_eq!(kind(message), expected, "{message}");
        }
    }

    /// The messages std writes today: a toolchain that changes them fails here, not in the log.
    #[test]
    fn kinds_of_real_panics() {
        fn kind_of(f: impl FnOnce() + std::panic::UnwindSafe) -> &'static str {
            let payload = std::panic::catch_unwind(f).unwrap_err();
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap();
            kind(&message)
        }
        let none: Option<u8> = std::hint::black_box(None);
        assert_eq!(
            kind_of(move || {
                let _ = none.unwrap();
            }),
            "unwrap_none"
        );
        let error: Result<u8, &str> = std::hint::black_box(Err("Marker"));
        assert_eq!(
            kind_of(move || {
                let _ = error.unwrap();
            }),
            "unwrap_err"
        );
        let items = [1];
        let index = std::hint::black_box(4);
        assert_eq!(
            kind_of(move || {
                let _ = items[index];
            }),
            "index_out_of_bounds"
        );
        let cell = std::cell::RefCell::new(1);
        assert_eq!(
            kind_of(std::panic::AssertUnwindSafe(|| {
                let _held = cell.borrow_mut();
                drop(cell.borrow_mut());
            })),
            "borrow"
        );
        let mutex = std::sync::Arc::new(std::sync::Mutex::new(1));
        let poisoner = mutex.clone();
        let _ = std::thread::spawn(move || {
            let _held = poisoner.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert_eq!(kind_of(move || drop(mutex.lock().unwrap())), "poisoned");
    }
}
