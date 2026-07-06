use crate::adb::AdbDevice;
use crate::log_entry::{DeviceInfo, DeviceSource};
use serde::Serialize;

#[allow(dead_code)]
pub const MOCK_DEVICE_ID: &str = "mock-device";
#[allow(dead_code)]
pub const MOCK_DEVICE_NAME: &str = "Mock Device";

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdbStatusMode {
    Bundled,
    MockFallback,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdbStatus {
    pub available: bool,
    pub mode: AdbStatusMode,
    pub path: Option<String>,
    pub message: String,
}

#[allow(dead_code)]
pub struct DeviceManager {
    adb_status: AdbStatus,
    devices: Vec<DeviceInfo>,
}

#[allow(dead_code)]
impl DeviceManager {
    pub fn mock_fallback(message: impl Into<String>) -> Self {
        Self {
            adb_status: AdbStatus {
                available: false,
                mode: AdbStatusMode::MockFallback,
                path: None,
                message: message.into(),
            },
            devices: vec![DeviceInfo {
                device_id: MOCK_DEVICE_ID.to_string(),
                device_name: MOCK_DEVICE_NAME.to_string(),
                connected: true,
                source: DeviceSource::Mock,
            }],
        }
    }

    pub fn from_adb_devices(path: String, adb_devices: Vec<AdbDevice>) -> Self {
        let count = adb_devices.len();
        Self {
            adb_status: AdbStatus {
                available: true,
                mode: AdbStatusMode::Bundled,
                path: Some(path),
                message: format!("ADB: {count} device(s) connected"),
            },
            devices: adb_devices
                .into_iter()
                .map(|device| DeviceInfo {
                    device_id: device.serial,
                    device_name: device.display_name,
                    connected: true,
                    source: DeviceSource::Adb,
                })
                .collect(),
        }
    }

    pub fn adb_status(&self) -> &AdbStatus {
        &self.adb_status
    }

    pub fn device_list(&self) -> &[DeviceInfo] {
        &self.devices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_mock_fallback_when_adb_is_missing() {
        let manager =
            DeviceManager::mock_fallback("ADB: missing libs/linux/adb, using mock device");

        assert_eq!(manager.adb_status().available, false);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::MockFallback);
        assert_eq!(manager.device_list().len(), 1);
        assert_eq!(manager.device_list()[0].device_id, MOCK_DEVICE_ID);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Mock);
    }

    #[test]
    fn builds_adb_devices_for_online_devices() {
        let devices = vec![
            AdbDevice {
                serial: "emulator-5554".to_string(),
                display_name: "Pixel 8".to_string(),
            },
            AdbDevice {
                serial: "R58N123ABC".to_string(),
                display_name: "Galaxy S23".to_string(),
            },
        ];

        let manager = DeviceManager::from_adb_devices("libs/linux/adb".to_string(), devices);

        assert_eq!(manager.adb_status().available, true);
        assert_eq!(manager.adb_status().mode, AdbStatusMode::Bundled);
        assert_eq!(manager.adb_status().message, "ADB: 2 device(s) connected");
        assert_eq!(manager.device_list().len(), 2);
        assert_eq!(manager.device_list()[0].source, DeviceSource::Adb);
        assert_eq!(manager.device_list()[1].device_name, "Galaxy S23");
    }
}
