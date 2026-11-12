//! The system the app runs on, for the launch record (`diagnostics-launch-and-crashes` design
//! decision 2): the OS's name and version, never anything of the machine or its user.

/// The operating system's name, such as `Windows 11`, `macOS` or `ubuntu`, and its version,
/// such as `10.0.26100`, `15.1` or `24.04/6.8.0-45-generic` (the distribution's version and the
/// kernel's). Unknown parts are empty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Os {
    pub name: String,
    pub version: String,
}

/// The OS this process runs on.
pub fn os() -> Os {
    imp::os()
}

/// Keeps a value the system reports to letters, digits and `.`, `_`, `-`, at most 32 of them:
/// a version or a distribution's id, never free text.
pub fn plain(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .take(32)
        .collect()
}

/// Windows 11 still reports itself as 10.0, from build 22000 on.
#[cfg_attr(not(windows), allow(dead_code))]
fn windows_name(major: u32, build: u32) -> &'static str {
    match (major, build) {
        (10, 22_000..) => "Windows 11",
        (10, _) => "Windows 10",
        _ => "Windows",
    }
}

/// `ID` and `VERSION_ID` from `/etc/os-release`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn os_release(text: &str) -> (String, String) {
    let value = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
            .map(|value| plain(value.trim().trim_matches('"')))
            .unwrap_or_default()
    };
    (value("ID"), value("VERSION_ID"))
}

#[cfg(windows)]
mod imp {
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;

    pub fn os() -> super::Os {
        // SAFETY: a zeroed OSVERSIONINFOW with its size set is what RtlGetVersion fills in.
        let info = unsafe {
            let mut info: OSVERSIONINFOW = std::mem::zeroed();
            info.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
            if RtlGetVersion(&mut info) != 0 {
                return super::Os { name: "Windows".into(), version: String::new() };
            }
            info
        };
        let (major, minor, build) = (info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber);
        super::Os {
            name: super::windows_name(major, build).into(),
            version: format!("{major}.{minor}.{build}"),
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    pub fn os() -> super::Os {
        let mut buffer = [0u8; 64];
        let mut len = buffer.len();
        // SAFETY: the name is NUL-terminated and the buffer's length is passed with it.
        let ok = unsafe {
            libc::sysctlbyname(
                c"kern.osproductversion".as_ptr(),
                buffer.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        } == 0;
        let version = if ok {
            let text = &buffer[..len.min(buffer.len())];
            let end = text.iter().position(|&b| b == 0).unwrap_or(text.len());
            super::plain(&String::from_utf8_lossy(&text[..end]))
        } else {
            String::new()
        };
        super::Os { name: "macOS".into(), version }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    pub fn os() -> super::Os {
        let release = std::fs::read_to_string("/etc/os-release")
            .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
            .unwrap_or_default();
        let (id, version) = super::os_release(&release);
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
        let kernel = super::plain(kernel.trim());
        let version = [version, kernel].into_iter().filter(|part| !part.is_empty());
        super::Os {
            name: if id.is_empty() { "linux".into() } else { id },
            version: version.collect::<Vec<_>>().join("/"),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod imp {
    pub fn os() -> super::Os {
        super::Os { name: std::env::consts::OS.into(), version: String::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_os_has_a_name_and_a_version_without_the_machine_in_it() {
        let os = os();
        assert!(!os.name.is_empty());
        if cfg!(any(windows, target_os = "macos", target_os = "linux")) {
            assert!(os.version.chars().any(|c| c.is_ascii_digit()), "{os:?}");
        }
        let machine =
            [std::env::var("USER"), std::env::var("USERNAME"), std::env::var("COMPUTERNAME")];
        for name in machine.into_iter().flatten().filter(|name| name.len() > 2) {
            assert!(!os.name.contains(&name) && !os.version.contains(&name), "{os:?}");
        }
    }

    #[test]
    fn windows_11_is_build_22000_and_later() {
        assert_eq!(windows_name(10, 19_045), "Windows 10");
        assert_eq!(windows_name(10, 22_000), "Windows 11");
        assert_eq!(windows_name(10, 26_100), "Windows 11");
        assert_eq!(windows_name(6, 9_600), "Windows");
    }

    #[test]
    fn os_release_keeps_the_id_and_version_only() {
        let text =
            "NAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nID=ubuntu\nPRETTY_NAME=\"Ubuntu 24.04 LTS\"\n";
        assert_eq!(os_release(text), ("ubuntu".into(), "24.04".into()));
        assert_eq!(os_release(""), (String::new(), String::new()));
        assert_eq!(os_release("ID=\"my distro; $(x)\"\n").0, "mydistrox");
    }
}
