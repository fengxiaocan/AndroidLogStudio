use anyhow::Context;
use chrono::{Local, Timelike};
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RecorderConfig {
    pub enabled: bool,
    pub root: PathBuf,
    pub device_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderStatus {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub warning: Option<String>,
}

pub struct Recorder {
    config: RecorderConfig,
    current_path: Option<PathBuf>,
}

impl Recorder {
    pub fn new(config: RecorderConfig) -> Self {
        Self {
            config,
            current_path: None,
        }
    }

    pub fn write_line(&mut self, line: &str) -> anyhow::Result<RecorderStatus> {
        if !self.config.enabled {
            return Ok(RecorderStatus {
                enabled: false,
                path: None,
                warning: None,
            });
        }

        let path = self.current_hour_path();
        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .with_context(|| format!("create recorder directory {}", parent.display()))?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open recorder file {}", path.display()))?;
        writeln!(file, "{line}")
            .with_context(|| format!("write recorder file {}", path.display()))?;
        self.current_path = Some(path.clone());

        Ok(RecorderStatus {
            enabled: true,
            path: Some(path),
            warning: None,
        })
    }

    fn current_hour_path(&self) -> PathBuf {
        let now = Local::now();
        let day = now.format("%Y-%m-%d").to_string();
        let hour = format!("{:02}.log", now.hour());
        sanitize_path(&self.config.root, &day, &self.config.device_name, &hour)
    }
}

fn sanitize_path(root: &Path, day: &str, device: &str, file: &str) -> PathBuf {
    let safe_device = device
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    root.join(day).join(safe_device).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_log_line_to_device_directory() {
        let dir = tempdir().expect("tempdir");
        let mut recorder = Recorder::new(RecorderConfig {
            enabled: true,
            root: dir.path().to_path_buf(),
            device_name: "Pixel 9".to_string(),
        });

        let status = recorder.write_line("hello log").expect("write succeeds");

        assert!(status.enabled);
        let path = status.path.expect("path set");
        assert!(path.display().to_string().contains("Pixel_9"));
        let content = std::fs::read_to_string(path).expect("read log");
        assert!(content.contains("hello log"));
    }

    #[test]
    fn disabled_recorder_does_not_write() {
        let dir = tempdir().expect("tempdir");
        let mut recorder = Recorder::new(RecorderConfig {
            enabled: false,
            root: dir.path().to_path_buf(),
            device_name: "Pixel".to_string(),
        });

        let status = recorder.write_line("hello log").expect("disabled succeeds");

        assert!(!status.enabled);
        assert!(status.path.is_none());
    }
}
