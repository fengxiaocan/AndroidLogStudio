use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AdbPaths {
    pub adb: PathBuf,
}

pub fn resolve_adb_path(project_root: &Path) -> AdbPaths {
    let relative = if cfg!(target_os = "windows") {
        PathBuf::from("tools/windows/adb.exe")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("tools/macos/adb")
    } else {
        PathBuf::from("tools/linux/adb")
    };

    AdbPaths {
        adb: project_root.join(relative),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_platform_adb_under_tools() {
        let paths = resolve_adb_path(Path::new("/app"));
        assert!(paths.adb.display().to_string().contains("tools"));
        assert!(paths.adb.display().to_string().contains("adb"));
    }
}
