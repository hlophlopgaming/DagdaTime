use super::*;

impl Storage {
    /// Delete [from,to), retaining the outside parts of events and tag ranges.
    pub fn delete_history(&mut self, from: i64, to: i64) -> Result<(), StorageError> {
        if from < 0 || to <= from {
            return Err(StorageError::InvalidRecord("invalid deletion range".into()));
        }
        self.flush()?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (table, columns, values) in [
            ("events", "bucket,data", "bucket,data"),
            (
                "tag_ranges",
                "tag_id,note,precedence_id",
                "tag_id,note,COALESCE(precedence_id,id)",
            ),
        ] {
            tx.execute(&format!("INSERT INTO {table}({columns},start_ts,end_ts) SELECT {values},?2,end_ts FROM {table} WHERE start_ts<?1 AND end_ts>?2"), params![from,to])?;
            tx.execute(
                &format!("DELETE FROM {table} WHERE start_ts>=?1 AND start_ts<?2 AND end_ts<=?2"),
                params![from, to],
            )?;
            tx.execute(
                &format!(
                    "UPDATE {table} SET end_ts=?1 WHERE start_ts<?1 AND end_ts>?1 AND start_ts<?2"
                ),
                params![from, to],
            )?;
            tx.execute(&format!("UPDATE {table} SET start_ts=?2 WHERE start_ts>=?1 AND start_ts<?2 AND end_ts>?2"), params![from,to])?;
        }
        tx.commit()?;
        self.tails.clear();
        self.auto_tail = None;
        self.metadata_version = self.metadata_version.wrapping_add(1);
        Ok(())
    }
}
