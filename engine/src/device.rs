use crate::filter::FilterQuery;
use crate::log_entry::{LogEntry, StatisticsSnapshot};
use crate::parser::parse_threadtime_line;
use crate::recorder::{Recorder, RecorderStatus};
use crate::ring_buffer::RingBuffer;
use crate::statistics::Statistics;

#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub logs: Vec<LogEntry>,
    pub stats: StatisticsSnapshot,
    pub recorder_status: RecorderStatus,
}

pub struct DeviceContext {
    pub device_id: String,
    pub device_name: String,
    seq: u64,
    filter: FilterQuery,
    buffer: RingBuffer<LogEntry>,
    statistics: Statistics,
    recorder: Recorder,
    recorder_status: RecorderStatus,
}

impl DeviceContext {
    pub fn new(
        device_id: String,
        device_name: String,
        buffer_capacity: usize,
        recorder: Recorder,
    ) -> Self {
        Self {
            device_id,
            device_name,
            seq: 0,
            filter: FilterQuery::default(),
            buffer: RingBuffer::new(buffer_capacity),
            statistics: Statistics::default(),
            recorder,
            recorder_status: RecorderStatus {
                enabled: false,
                path: None,
                warning: None,
            },
        }
    }

    pub fn set_filter(&mut self, query: FilterQuery) {
        self.filter = query;
    }

    pub fn ingest_line(&mut self, raw_line: &str) -> Option<LogEntry> {
        self.seq += 1;
        let mut entry = parse_threadtime_line(self.seq, raw_line)?;
        entry.hidden = !self.filter.matches(&entry);
        self.statistics.observe(&entry);
        self.recorder_status =
            self.recorder
                .write_line(raw_line)
                .unwrap_or_else(|error| RecorderStatus {
                    enabled: false,
                    path: None,
                    warning: Some(error.to_string()),
                });
        self.buffer.push(entry.clone());
        Some(entry)
    }

    pub fn latest_visible_snapshot(&self, limit: usize) -> DeviceSnapshot {
        let logs = self
            .buffer
            .latest(limit)
            .into_iter()
            .filter(|entry| !entry.hidden)
            .collect();

        DeviceSnapshot {
            logs,
            stats: self.statistics.snapshot(),
            recorder_status: self.recorder_status.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::RecorderConfig;
    use tempfile::tempdir;

    #[test]
    fn ingests_and_snapshots_visible_lines() {
        let dir = tempdir().expect("tempdir");
        let recorder = Recorder::new(RecorderConfig {
            enabled: false,
            root: dir.path().to_path_buf(),
            device_name: "mock".to_string(),
        });
        let mut device = DeviceContext::new("mock".to_string(), "Mock".to_string(), 10, recorder);
        device.set_filter(FilterQuery::parse("level:error"));

        device.ingest_line("07-04 12:34:56.789  1234  5678 I Tag: info");
        device.ingest_line("07-04 12:34:57.789  1234  5678 E Tag: error");

        let snapshot = device.latest_visible_snapshot(500);
        assert_eq!(snapshot.logs.len(), 1);
        assert_eq!(snapshot.logs[0].message, "error");
        assert_eq!(snapshot.stats.errors, 1);
    }
}
