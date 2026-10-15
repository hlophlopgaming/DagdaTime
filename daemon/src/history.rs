use crate::Shared;
use draugr_time_core::{Archive, Report, StorageError};
use std::collections::{BTreeMap, HashSet};

impl Shared {
    // Release storage between complete prefixes, and do aggregation outside the lock.
    fn history_chunks(
        &self,
        from: i64,
        to: i64,
        mut consume: impl FnMut(Archive, i64, i64) -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        if from < 0 || to <= from {
            return Err(StorageError::InvalidRecord(
                "expected 0 <= from < to".into(),
            ));
        }
        let mut revision = None;
        let mut start = from;
        while start < to {
            let (archive, end) = {
                let db = self.storage.lock().unwrap();
                let current = db.metadata_version()?;
                if revision.is_some_and(|version| version != current) {
                    return Err(StorageError::InvalidRecord(
                        "History changed while reading; reload and try again".into(),
                    ));
                }
                revision = Some(current);
                let (_, end) = db.event_chunk(None, start, to, 8193)?;
                let end = end.max(start + 1);
                (db.export_archive(start, end)?, end)
            };
            consume(archive, start, end)?;
            start = end;
        }
        if revision != Some(self.storage.lock().unwrap().metadata_version()?) {
            return Err(StorageError::InvalidRecord(
                "History changed while reading; reload and try again".into(),
            ));
        }
        Ok(())
    }

    pub fn report(&self, from: i64, to: i64) -> Result<Report, StorageError> {
        let mut report = Report {
            from,
            to,
            ..Default::default()
        };
        let mut strings = HashSet::new();
        self.history_chunks(from, to, |archive, start, end| {
            let chunk = Report::calculate(
                &archive.events,
                &archive.tags,
                &archive.tag_ranges,
                start,
                end,
            )?;
            report.active_ms += chunk.active_ms;
            for mut interval in chunk.intervals {
                for value in [
                    &mut interval.tag,
                    &mut interval.application,
                    &mut interval.document,
                ] {
                    if let Some(existing) = strings.get(value) {
                        *value = std::sync::Arc::clone(existing);
                    } else {
                        strings.insert(value.clone());
                    }
                }
                if let Some(last) = report.intervals.last_mut().filter(|last| {
                    last.end_ts == interval.start_ts
                        && last.tag_id == interval.tag_id
                        && last.application == interval.application
                        && last.document == interval.document
                }) {
                    last.end_ts = interval.end_ts;
                } else {
                    report.intervals.push(interval);
                }
            }
            Ok(())
        })?;
        Ok(report)
    }

    pub fn export_archive(&self, from: i64, to: i64) -> Result<Archive, StorageError> {
        let mut events: BTreeMap<i64, draugr_time_core::Event> = BTreeMap::new();
        let mut ranges: BTreeMap<i64, draugr_time_core::TagRange> = BTreeMap::new();
        let mut tags = Vec::new();
        self.history_chunks(from, to, |archive, _, _| {
            tags = archive.tags;
            for event in archive.events {
                events
                    .entry(event.id)
                    .and_modify(|old| old.end_ts = old.end_ts.max(event.end_ts))
                    .or_insert(event);
            }
            for range in archive.tag_ranges {
                ranges
                    .entry(range.id)
                    .and_modify(|old| old.end_ts = old.end_ts.max(range.end_ts))
                    .or_insert(range);
            }
            Ok(())
        })?;
        let mut events: Vec<_> = events.into_values().collect();
        events.sort_by_key(|e| (e.start_ts, e.id));
        let mut tag_ranges: Vec<_> = ranges.into_values().collect();
        tag_ranges.sort_by_key(|r| (r.start_ts, r.id));
        Ok(Archive {
            format: "draugr-time-v1".into(),
            events,
            tags,
            tag_ranges,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_history_edit_rejects_mixed_export() {
        let mut db = draugr_time_core::Storage::open_in_memory().unwrap();
        for i in 0..8300 {
            db.record_interval(
                "afk",
                &serde_json::json!({"status":"active"}),
                i * 10 + 1,
                i * 10 + 9,
            )
            .unwrap();
        }
        let shared = Shared::new(db, "test".into(), 300);
        let mut calls = 0;
        let result = shared.history_chunks(0, 83_000, |_, _, _| {
            calls += 1;
            shared
                .storage
                .lock()
                .unwrap()
                .tag_interval("New", "", 1, 2, false)?;
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
    }
    #[test]
    fn aggregation_runs_without_storage_or_control_locked() {
        let shared = Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let mut called = false;
        shared
            .history_chunks(0, 100, |_, _, _| {
                assert!(shared.storage.try_lock().is_ok());
                assert!(shared.control.try_lock().is_ok());
                called = true;
                Ok(())
            })
            .unwrap();
        assert!(called);
    }
}
