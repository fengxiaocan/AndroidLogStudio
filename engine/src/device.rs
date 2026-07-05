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
        let mut hidden = 0;

        for entry in self.buffer.iter_mut() {
            entry.hidden = !self.filter.matches(entry);
            if entry.hidden {
                hidden += 1;
            }
        }

        self.statistics.set_hidden(hidden);
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
        let mut logs = self
            .buffer
            .latest(usize::MAX)
            .into_iter()
            .filter(|entry| !entry.hidden)
            .rev()
            .take(limit)
            .collect::<Vec<_>>();
        logs.reverse();

        DeviceSnapshot {
            logs,
            stats: self.statistics.snapshot(),
            recorder_status: self.recorder_status.clone(),
        }
    }

    pub fn search_visible_sequences(&self, query: &str) -> Vec<u64> {
        if query.is_empty() {
            return Vec::new();
        }

        let query = query.to_lowercase();
        self.buffer
            .latest(1_000_000)
            .into_iter()
            .filter(|entry| !entry.hidden && entry.message.to_lowercase().contains(&query))
            .map(|entry| entry.seq)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::RecorderConfig;
    use tempfile::tempdir;

    fn new_test_device(buffer_capacity: usize) -> DeviceContext {
        let dir = tempdir().expect("tempdir");
        let recorder = Recorder::new(RecorderConfig {
            enabled: false,
            root: dir.path().to_path_buf(),
            device_name: "mock".to_string(),
        });
        DeviceContext::new(
            "mock".to_string(),
            "Mock".to_string(),
            buffer_capacity,
            recorder,
        )
    }

    #[test]
    fn ingests_and_snapshots_visible_lines() {
        let mut device = new_test_device(10);
        device.set_filter(FilterQuery::parse("level:error"));

        device.ingest_line("07-04 12:34:56.789  1234  5678 I Tag: info");
        device.ingest_line("07-04 12:34:57.789  1234  5678 E Tag: error");

        let snapshot = device.latest_visible_snapshot(500);
        assert_eq!(snapshot.logs.len(), 1);
        assert_eq!(snapshot.logs[0].message, "error");
        assert_eq!(snapshot.stats.errors, 1);
    }

    #[test]
    fn limits_after_filtering_visible_lines() {
        let mut device = new_test_device(5);
        device.set_filter(FilterQuery::parse("level:error"));

        device.ingest_line("07-04 12:34:56.000  1234  5678 E Tag: first error");
        device.ingest_line("07-04 12:34:57.000  1234  5678 E Tag: second error");
        device.ingest_line("07-04 12:34:58.000  1234  5678 E Tag: third error");
        device.ingest_line("07-04 12:34:59.000  1234  5678 I Tag: hidden info one");
        device.ingest_line("07-04 12:35:00.000  1234  5678 I Tag: hidden info two");

        let snapshot = device.latest_visible_snapshot(2);

        assert_eq!(snapshot.logs.len(), 2);
        assert_eq!(snapshot.logs[0].message, "second error");
        assert_eq!(snapshot.logs[1].message, "third error");
    }

    #[test]
    fn searches_visible_messages_case_insensitively() {
        let mut device = new_test_device(10);
        device.set_filter(FilterQuery::parse("level:error"));

        device.ingest_line("07-04 12:34:56.000  1234  5678 E Tag: Alpha failure");
        device.ingest_line("07-04 12:34:57.000  1234  5678 I Tag: alpha hidden");
        device.ingest_line("07-04 12:34:58.000  1234  5678 E Tag: beta ALPHA");

        assert_eq!(device.search_visible_sequences("alpha"), vec![1, 3]);
        assert!(device.search_visible_sequences("").is_empty());
    }

    #[test]
    fn set_filter_recomputes_buffer_visibility_and_hidden_count() {
        let mut device = new_test_device(10);

        device.ingest_line("07-04 12:34:56.000  1234  5678 I Tag: info");
        device.ingest_line("07-04 12:34:57.000  1234  5678 E Tag: error");
        device.set_filter(FilterQuery::parse("level:error"));

        let snapshot = device.latest_visible_snapshot(500);
        assert_eq!(snapshot.logs.len(), 1);
        assert_eq!(snapshot.logs[0].message, "error");
        assert_eq!(snapshot.stats.hidden, 1);
    }
}
