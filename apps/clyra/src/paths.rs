//! Application storage paths. Provider credentials keep their own locations.

use std::ffi::OsString;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    resolve_data_dir(|name| std::env::var_os(name))
}

fn resolve_data_dir(mut env: impl FnMut(&str) -> Option<OsString>) -> PathBuf {
    if let Some(dir) = env("CLYRA_DATA_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    // Legacy override from the Clyra era.
    if let Some(dir) = env("ZERON_DATA_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    {
        // Explorer does not set HOME. Do not let a shell-specific HOME select
        // a different workspace from a desktop launch, or migrate credentials
        // between Unix-style and native Windows directories implicitly.
        let local = env("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env("USERPROFILE")
                    .filter(|value| !value.is_empty())
                    .map(|home| PathBuf::from(home).join("AppData").join("Local"))
            })
            .expect("LOCALAPPDATA and USERPROFILE not set; set CLYRA_DATA_DIR");
        let dir = local.join("Clyra");
        // One-shot rebrand migration: adopt the legacy Zeron data dir.
        if !dir.exists() {
            let old = local.join("Zeron");
            if old.exists() && std::fs::rename(&old, &dir).is_ok() {
                eprintln!("migrated data dir {} -> {}", old.display(), dir.display());
            }
        }
        dir
    }
    #[cfg(not(windows))]
    {
        let home = PathBuf::from(env("HOME").expect("HOME not set"));
        let dir = home.join(".clyra");
        // One-shot rebrand migration: adopt the Clyra data dir (which
        // itself adopted the pre-rename `.comet-native` dir in 0.2.0).
        if !dir.exists() {
            for old in [home.join(".zeron"), home.join(".comet-native")] {
                if old.exists() && std::fs::rename(&old, &dir).is_ok() {
                    eprintln!("migrated data dir {} -> {}", old.display(), dir.display());
                    break;
                }
            }
        }
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(vars: &[(&str, &str)]) -> PathBuf {
        resolve_data_dir(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.into())
        })
    }

    #[test]
    fn explicit_data_dir_needs_no_home() {
        assert_eq!(
            resolve(&[("CLYRA_DATA_DIR", "custom data")]),
            PathBuf::from("custom data")
        );
    }

    #[test]
    fn legacy_data_dir_override_still_wins() {
        assert_eq!(
            resolve(&[("ZERON_DATA_DIR", "legacy data")]),
            PathBuf::from("legacy data")
        );
    }

    #[cfg(windows)]
    #[test]
    fn explorer_launch_without_home_uses_local_app_data() {
        assert_eq!(
            resolve(&[("LOCALAPPDATA", r"C:\Users\Test User\AppData\Local")]),
            PathBuf::from(r"C:\Users\Test User\AppData\Local\Clyra"),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_profile_fallback_handles_unicode_and_apostrophes() {
        assert_eq!(
            resolve(&[("USERPROFILE", r"C:\Users\O'Brien 日本語")]),
            PathBuf::from(r"C:\Users\O'Brien 日本語\AppData\Local\Clyra"),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_default_does_not_depend_on_shell_home() {
        assert_eq!(
            resolve(&[("HOME", r"D:\msys-home"), ("LOCALAPPDATA", r"C:\Local")]),
            PathBuf::from(r"C:\Local\Clyra"),
        );
    }
}
