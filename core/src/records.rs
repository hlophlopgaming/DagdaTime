use super::*;
use rusqlite::types::Value as SqlValue;

#[derive(Debug, Clone, Copy)]
pub enum Collection {
    Tags,
    TagRanges,
    Rules,
}

impl Collection {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "tags" => Some(Self::Tags),
            "tag_ranges" => Some(Self::TagRanges),
            "rules" => Some(Self::Rules),
            _ => None,
        }
    }
    fn table(self) -> &'static str {
        match self {
            Self::Tags => "tags",
            Self::TagRanges => "tag_ranges",
            Self::Rules => "rules",
        }
    }
    fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Tags => &["name", "color", "parent_id"],
            Self::TagRanges => &["tag_id", "start_ts", "end_ts", "note"],
            Self::Rules => &["field", "pattern", "tag_id", "priority"],
        }
    }
    fn select(self) -> String {
        let fields = std::iter::once("id")
            .chain(self.fields().iter().copied())
            .map(|field| format!("'{field}', {field}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("SELECT json_object({fields}) FROM {}", self.table())
    }
}

fn invalid(message: &str) -> StorageError {
    StorageError::InvalidRecord(message.into())
}

impl Storage {
    pub fn events(
        &self,
        bucket: Option<&str>,
        from: i64,
        to: i64,
    ) -> Result<Vec<Event>, StorageError> {
        if from < 0 || to <= from {
            return Err(invalid("expected 0 <= from < to"));
        }
        // Events in one bucket never overlap: heartbeats are nondecreasing and a new
        // event starts at or after the previous end. So the only event that can begin
        // before `from` and still reach into the range is the last one starting at or
        // before `from`. Seeking to it keeps the scan on idx_events_bucket_start and
        // proportional to the result, not to the whole history. Buckets are enumerated
        // with index skips when no bucket is given. Importers (M7) must keep the
        // invariant or this query needs an end_ts index instead.
        let mut statement = self.connection.prepare_cached(
            "WITH RECURSIVE buckets(name) AS (
                SELECT ?1 WHERE ?1 IS NOT NULL
                UNION ALL SELECT MIN(bucket) FROM events WHERE ?1 IS NULL
                UNION ALL SELECT (SELECT MIN(bucket) FROM events WHERE bucket > buckets.name)
                    FROM buckets WHERE ?1 IS NULL AND buckets.name IS NOT NULL
            )
            SELECT e.id, e.bucket, e.start_ts, e.end_ts, e.data
            FROM buckets JOIN events e ON e.bucket = buckets.name
                AND e.start_ts >= COALESCE((SELECT MAX(p.start_ts) FROM events p
                    WHERE p.bucket = buckets.name AND p.start_ts <= ?2), ?2)
                AND e.start_ts < ?3 AND e.end_ts > ?2
            ORDER BY e.start_ts, e.id LIMIT 100001",
        )?;
        let events = statement
            .query_map(params![bucket, from, to], event_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if events.len() > 100_000 {
            return Err(invalid("too many events; query a shorter interval"));
        }
        Ok(events)
    }

    /// Close a sampled interval only across a short, continuously observed gap.
    pub fn extend_last(&mut self, bucket: &str, ts: i64, max_gap: i64) -> Result<(), StorageError> {
        let batch = self.batch()?;
        batch.extend_last(bucket, ts, max_gap)?;
        batch.commit()
    }

    pub fn list(&self, collection: Collection) -> Result<Vec<Value>, StorageError> {
        let mut statement = self
            .connection
            .prepare(&format!("{} ORDER BY id", collection.select()))?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|row| Ok(serde_json::from_str(&row)?))
            .collect()
    }

    pub fn get(&self, collection: Collection, id: i64) -> Result<Value, StorageError> {
        let row: Option<String> = self
            .connection
            .query_row(
                &format!("{} WHERE id=?1", collection.select()),
                [id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(serde_json::from_str(&row.ok_or(StorageError::NotFound)?)?)
    }

    pub fn put(
        &mut self,
        collection: Collection,
        id: Option<i64>,
        mut data: Value,
    ) -> Result<Value, StorageError> {
        let object = data
            .as_object_mut()
            .ok_or_else(|| invalid("expected JSON object"))?;
        if id.is_some_and(|id| id <= 0)
            || object
                .keys()
                .any(|key| !collection.fields().contains(&key.as_str()))
        {
            return Err(invalid("invalid id or unknown field"));
        }
        match collection {
            Collection::Tags => {
                if !object
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|name| !name.trim().is_empty())
                {
                    return Err(invalid("tag name must be nonempty"));
                }
                object.entry("color").or_insert(Value::Null);
                object.entry("parent_id").or_insert(Value::Null);
                if let Some(color) = object["color"].as_str() {
                    if color.len() != 7
                        || !color.starts_with('#')
                        || !color[1..].bytes().all(|c| c.is_ascii_hexdigit())
                    {
                        return Err(invalid("color must be #RRGGBB"));
                    }
                } else if !object["color"].is_null() {
                    return Err(invalid("color must be a string or null"));
                }
            }
            Collection::TagRanges => {
                object.entry("note").or_insert(Value::Null);
                let start = object
                    .get("start_ts")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| invalid("start_ts must be Unix milliseconds"))?;
                let end = object
                    .get("end_ts")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| invalid("end_ts must be Unix milliseconds"))?;
                if start < 0 || end <= start {
                    return Err(invalid("range must have start < end"));
                }
                if !object["note"].is_null() && !object["note"].is_string() {
                    return Err(invalid("note must be string or null"));
                }
            }
            Collection::Rules => {
                object.entry("priority").or_insert(Value::from(0));
                if !object
                    .get("field")
                    .and_then(Value::as_str)
                    .is_some_and(|field| ["app", "title", "url"].contains(&field))
                    || !object.get("pattern").is_some_and(Value::is_string)
                    || !object["priority"].is_i64()
                {
                    return Err(invalid("invalid rule field, pattern or priority"));
                }
            }
        }
        for key in ["tag_id", "parent_id"] {
            if let Some(value) = object.get(key) {
                if !(value.is_null() && key == "parent_id")
                    && !value.as_i64().is_some_and(|id| id > 0)
                {
                    return Err(invalid("tag reference must be a positive integer"));
                }
            } else if key == "tag_id" && !matches!(collection, Collection::Tags) {
                return Err(invalid("tag_id required"));
            }
        }
        let values: Vec<SqlValue> = collection
            .fields()
            .iter()
            .map(|field| match object.get(*field) {
                Some(Value::String(value)) => Ok(SqlValue::Text(value.clone())),
                Some(Value::Number(value)) => value
                    .as_i64()
                    .map(SqlValue::Integer)
                    .ok_or_else(|| invalid("integer required")),
                Some(Value::Null) => Ok(SqlValue::Null),
                _ => Err(invalid("required field missing or invalid")),
            })
            .collect::<Result<_, _>>()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let (Collection::Tags, Some(id), Some(parent)) = (
            collection,
            id,
            object.get("parent_id").and_then(Value::as_i64),
        ) {
            let cycle: bool = transaction.query_row("WITH RECURSIVE children(id) AS (SELECT ?1 UNION SELECT tags.id FROM tags JOIN children ON tags.parent_id=children.id) SELECT EXISTS(SELECT 1 FROM children WHERE id=?2)", params![id,parent], |row| row.get(0))?;
            if cycle {
                return Err(invalid("tag parent would create a cycle"));
            }
        }
        let result_id = if let Some(id) = id {
            let fields = collection
                .fields()
                .iter()
                .enumerate()
                .map(|(i, field)| format!("{field}=?{}", i + 1))
                .collect::<Vec<_>>()
                .join(",");
            let mut values = values;
            values.push(SqlValue::Integer(id));
            if transaction.execute(
                &format!(
                    "UPDATE {} SET {fields} WHERE id=?{}",
                    collection.table(),
                    values.len()
                ),
                rusqlite::params_from_iter(values),
            )? == 0
            {
                return Err(StorageError::NotFound);
            }
            id
        } else {
            let placeholders = (1..=values.len())
                .map(|i| format!("?{i}"))
                .collect::<Vec<_>>()
                .join(",");
            transaction.execute(
                &format!(
                    "INSERT INTO {} ({}) VALUES ({placeholders})",
                    collection.table(),
                    collection.fields().join(",")
                ),
                rusqlite::params_from_iter(values),
            )?;
            transaction.last_insert_rowid()
        };
        transaction.commit()?;
        self.get(collection, result_id)
    }

    pub fn delete(&mut self, collection: Collection, id: i64) -> Result<bool, StorageError> {
        Ok(self.connection.execute(
            &format!("DELETE FROM {} WHERE id=?1", collection.table()),
            [id],
        )? > 0)
    }
}

pub(crate) fn extend_last_in(
    connection: &Connection,
    bucket: &str,
    ts: i64,
    max_gap: i64,
) -> Result<(), StorageError> {
    connection
        .prepare_cached("UPDATE events SET end_ts=?1 WHERE id=(SELECT id FROM events WHERE bucket=?2 ORDER BY start_ts DESC,id DESC LIMIT 1) AND ?1 >= end_ts AND ?1-end_ts < ?3")?
        .execute(params![ts, bucket, max_gap])?;
    Ok(())
}
