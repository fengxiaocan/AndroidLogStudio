use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbPaths {
    pub adb: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbDevice {
    pub serial: String,
    pub display_name: String,
}

pub fn resolve_adb_path(project_root: &Path) -> AdbPaths {
    let relative = if cfg!(target_os = "windows") {
        PathBuf::from("libs/windows/adb.exe")
    } else if cfg!(target_os = "macos") {
        PathBuf::from("libs/macos/adb")
    } else {
        PathBuf::from("libs/linux/adb")
    };

    AdbPaths {
        adb: project_root.join(relative),
    }
}

pub fn parse_devices_output(output: &str) -> Vec<AdbDevice> {
    output
        .lines()
        .skip(1)
        .filter_map(parse_device_line)
        .collect()
}

fn parse_device_line(line: &str) -> Option<AdbDevice> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.split_whitespace();
    let serial = parts.next()?.to_string();
    let state = parts.next()?;
    if state != "device" {
        return None;
    }

    let model = parts
        .find_map(|part| part.strip_prefix("model:"))
        .map(|model| model.replace('_', " "))
        .unwrap_or_else(|| serial.clone());

    Some(AdbDevice {
        serial,
        display_name: model,
    })
}

pub async fn list_devices(adb_path: &Path) -> anyhow::Result<Vec<AdbDevice>> {
    let output = Command::new(adb_path)
        .arg("devices")
        .arg("-l")
        .output()
        .await?;
    if !output.status.success() {
        anyhow::bail!("adb devices -l exited with {}", output.status);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_devices_output(&stdout))
}

pub fn logcat_command(adb_path: &Path, serial: &str) -> Command {
    let mut command = Command::new(adb_path);
    command
        .arg("-s")
        .arg(serial)
        .arg("logcat")
        .arg("-v")
        .arg("threadtime")
        // -T 1 streams from the latest line instead of dumping the entire
        // device ring buffer first (which floods the UI with old logs).
        .arg("-T")
        .arg("1");
    command
}

/// Fetch PID → process/package names for package enrichment (Android Studio style).
pub async fn list_process_packages(adb_path: &Path, serial: &str) -> anyhow::Result<String> {
    // Prefer modern toybox columns; fall back to plain `ps -A`.
    let primary = Command::new(adb_path)
        .arg("-s")
        .arg(serial)
        .arg("shell")
        .arg("ps")
        .arg("-A")
        .arg("-o")
        .arg("PID,NAME")
        .output()
        .await?;

    if primary.status.success() {
        let stdout = String::from_utf8_lossy(&primary.stdout).into_owned();
        if stdout.lines().count() > 1 {
            return Ok(stdout);
        }
    }

    let fallback = Command::new(adb_path)
        .arg("-s")
        .arg(serial)
        .arg("shell")
        .arg("ps")
        .arg("-A")
        .output()
        .await?;
    if !fallback.status.success() {
        anyhow::bail!("adb shell ps failed with {}", fallback.status);
    }
    Ok(String::from_utf8_lossy(&fallback.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn resolves_platform_adb_under_libs() {
        let paths = resolve_adb_path(Path::new("/app"));
        let rendered = paths.adb.display().to_string();

        assert!(rendered.contains("libs"));
        assert!(rendered.contains("adb"));
        assert!(!rendered.contains("tools"));
    }

    #[test]
    fn parses_online_devices_with_model_name() {
        let output = "List of devices attached\n\
emulator-5554 device product:sdk_gphone64_x86_64 model:Pixel_8 device:emu64 transport_id:1\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[0].display_name, "Pixel 8");
    }

    #[test]
    fn parses_multiple_online_devices() {
        let output = "List of devices attached\n\
emulator-5554 device model:Pixel_8\n\
R58N123ABC device model:Galaxy_S23\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].serial, "emulator-5554");
        assert_eq!(devices[1].serial, "R58N123ABC");
    }

    #[test]
    fn ignores_non_online_devices() {
        let output = "List of devices attached\n\
emulator-5554 offline model:Pixel_8\n\
R58N123ABC unauthorized model:Galaxy_S23\n\
ZX1 recovery model:Recovery_Device\n\
OK1 device model:Online_Device\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].serial, "OK1");
        assert_eq!(devices[0].display_name, "Online Device");
    }

    #[test]
    fn uses_serial_when_model_is_missing() {
        let output = "List of devices attached\nabc123 device usb:1-1\n";

        let devices = parse_devices_output(output);

        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].display_name, "abc123");
    }

    #[test]
    fn logcat_command_starts_from_latest_lines_only() {
        let command = logcat_command(Path::new("/tmp/adb"), "serial-1");
        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        // -T 1: print the most recent line, then keep streaming new ones.
        // Without -T, adb dumps the entire ring buffer first (old flood).
        assert_eq!(
            args,
            vec![
                "-s",
                "serial-1",
                "logcat",
                "-v",
                "threadtime",
                "-T",
                "1",
            ]
        );
    }
}
