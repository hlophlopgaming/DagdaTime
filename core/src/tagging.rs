use super::*;

#[derive(Clone, Debug, PartialEq)]
struct TagState {
    tags: Vec<Value>,
    ranges: Vec<Value>,
    rules: Vec<Value>,
}
#[derive(Clone, Debug)]
pub struct TagUndo {
    before: TagState,
    after: TagState,
}
fn invalid(message: &str) -> StorageError {
    StorageError::InvalidRecord(message.into())
}
impl Storage {
    fn tag_state(&self) -> Result<TagState, StorageError> {
        Ok(TagState {
            tags: self.list(Collection::Tags)?,
            ranges: self.list(Collection::TagRanges)?,
            rules: self.list(Collection::Rules)?,
        })
    }
    fn tag_change(
        &mut self,
        change: impl FnOnce(&Connection) -> Result<(), StorageError>,
    ) -> Result<TagUndo, StorageError> {
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let before = self.tag_state()?;
            change(&self.connection)?;
            let after = self.tag_state()?;
            Ok(TagUndo { before, after })
        })();
        match result {
            Ok(undo) => {
                if let Err(error) = self.connection.execute_batch("COMMIT") {
                    self.connection.execute_batch("ROLLBACK")?;
                    return Err(error.into());
                }
                self.metadata_version = self.metadata_version.wrapping_add(1);
                Ok(undo)
            }
            Err(error) => {
                self.connection.execute_batch("ROLLBACK")?;
                Err(error)
            }
        }
    }
    pub fn undo_tags(&mut self, undo: TagUndo) -> Result<(), StorageError> {
        self.tag_change(|connection| {
            // Compare inside the write transaction so API edits cannot be lost.
            let current = |collection: Collection| -> Result<Vec<Value>, StorageError> {
                let mut query=connection.prepare(&format!("{} ORDER BY id",collection.select()))?;
                let rows = query.query_map([], |r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
                rows.into_iter().map(|r| Ok(serde_json::from_str(&r)?)).collect()
            };
            if (TagState { tags: current(Collection::Tags)?, ranges: current(Collection::TagRanges)?, rules: current(Collection::Rules)? }) != undo.after { return Err(invalid("Tags changed since this action; undo refused to protect newer edits")); }
            connection.execute_batch("PRAGMA defer_foreign_keys=ON; DELETE FROM rules; DELETE FROM tag_ranges; DELETE FROM tags;")?;
            for tag in &undo.before.tags {
                connection.execute("INSERT INTO tags(id,name,color,parent_id,archived) VALUES(?1,?2,?3,?4,?5)", params![tag["id"].as_i64(),tag["name"].as_str(),tag["color"].as_str(),tag["parent_id"].as_i64(),tag["archived"].as_i64()])?;
            }
            for range in &undo.before.ranges {
                connection.execute("INSERT INTO tag_ranges(id,tag_id,start_ts,end_ts,note) VALUES(?1,?2,?3,?4,?5)", params![range["id"].as_i64(),range["tag_id"].as_i64(),range["start_ts"].as_i64(),range["end_ts"].as_i64(),range["note"].as_str()])?;
            }
            for rule in &undo.before.rules {
                connection.execute("INSERT INTO rules(id,field,pattern,tag_id,priority) VALUES(?1,?2,?3,?4,?5)", params![rule["id"].as_i64(),rule["field"].as_str(),rule["pattern"].as_str(),rule["tag_id"].as_i64(),rule["priority"].as_i64()])?;
            }
            Ok(())
        })?;
        Ok(())
    }
    /// Replace removes intersecting blocks; split preserves their outside portions.
    pub fn tag_interval(
        &mut self,
        path: &str,
        note: &str,
        start: i64,
        end: i64,
        replace: bool,
    ) -> Result<TagUndo, StorageError> {
        self.edit_tag_interval(None, path, note, start, end, replace)
    }
    pub fn edit_tag_interval(
        &mut self,
        id: Option<i64>,
        path: &str,
        note: &str,
        start: i64,
        end: i64,
        replace: bool,
    ) -> Result<TagUndo, StorageError> {
        let names: Vec<_> = path.split(',').map(str::trim).collect();
        if start < 0 || end <= start || names.iter().any(|s| s.is_empty()) {
            return Err(invalid("Use nonempty comma-separated tags and start < end"));
        }
        self.tag_change(|connection| {
            if let Some(id) = id { if connection.execute("DELETE FROM tag_ranges WHERE id=?1", [id])? == 0 { return Err(StorageError::NotFound); } }
            let mut parent: Option<i64> = None;
            for name in names {
                let existing = connection.query_row("SELECT id FROM tags WHERE name=?1 AND parent_id IS ?2 AND archived=0 ORDER BY id LIMIT 1", params![name,parent], |r|r.get::<_,i64>(0)).optional()?;
                parent = Some(if let Some(id) = existing { id } else {
                    connection.execute("INSERT INTO tags(name,parent_id) VALUES(?1,?2)", params![name,parent])?;
                    connection.last_insert_rowid()
                });
            }
            let mut query = connection.prepare("SELECT id,tag_id,start_ts,end_ts,note FROM tag_ranges WHERE start_ts<?2 AND end_ts>?1")?;
            let overlaps = query.query_map(params![start,end], |r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,i64>(2)?,r.get::<_,i64>(3)?,r.get::<_,Option<String>>(4)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for (id,tag,from,to,note) in overlaps {
                connection.execute("DELETE FROM tag_ranges WHERE id=?1", [id])?;
                if !replace {
                    for (a,b) in [(from,start.min(to)),(end.max(from),to)] {
                        if a < b { connection.execute("INSERT INTO tag_ranges(tag_id,start_ts,end_ts,note) VALUES(?1,?2,?3,?4)", params![tag,a,b,note])?; }
                    }
                }
            }
            connection.execute("INSERT INTO tag_ranges(tag_id,start_ts,end_ts,note) VALUES(?1,?2,?3,?4)", params![parent,start,end,note])?;
            Ok(())
        })
    }
    pub fn delete_tag_range(&mut self, id: i64) -> Result<TagUndo, StorageError> {
        self.tag_change(|c| {
            if c.execute("DELETE FROM tag_ranges WHERE id=?1", [id])? == 0 {
                return Err(StorageError::NotFound);
            }
            Ok(())
        })
    }
    pub fn update_tag(
        &mut self,
        id: i64,
        name: &str,
        color: Option<&str>,
        archived: bool,
    ) -> Result<TagUndo, StorageError> {
        if name.trim().is_empty()
            || color.is_some_and(|c| {
                c.len() != 7
                    || !c.starts_with('#')
                    || !c[1..].bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err(invalid("Nonempty name and #RRGGBB color required"));
        }
        self.tag_change(|c| {
            if c.execute(
                "UPDATE tags SET name=?2,color=?3,archived=?4 WHERE id=?1",
                params![id, name.trim(), color, archived],
            )? == 0
            {
                return Err(StorageError::NotFound);
            }
            Ok(())
        })
    }
    pub fn merge_tags(&mut self, source: i64, target: i64) -> Result<TagUndo, StorageError> {
        if source == target {
            return Err(invalid("Select another tag to merge into"));
        }
        self.tag_change(|c| {
            let cycle: bool = c.query_row("WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT tags.id FROM tags JOIN tree ON parent_id=tree.id) SELECT EXISTS(SELECT 1 FROM tree WHERE id=?2)", params![source,target], |r|r.get(0))?;
            if cycle { return Err(invalid("Cannot merge into a descendant")); }
            let exists: bool = c.query_row("SELECT EXISTS(SELECT 1 FROM tags WHERE id=?1)", [target], |r|r.get(0))?;
            if !exists { return Err(StorageError::NotFound); }
            c.execute("UPDATE tag_ranges SET tag_id=?2 WHERE tag_id=?1", params![source,target])?;
            c.execute("UPDATE rules SET tag_id=?2 WHERE tag_id=?1", params![source,target])?;
            c.execute("UPDATE tags SET parent_id=?2 WHERE parent_id=?1", params![source,target])?;
            if c.execute("DELETE FROM tags WHERE id=?1", [source])? == 0 { return Err(StorageError::NotFound); }
            Ok(())
        })
    }
    pub fn delete_tag_tree(&mut self, id: i64) -> Result<TagUndo, StorageError> {
        self.tag_change(|c| {
            let tree = "WITH RECURSIVE tree(id) AS (SELECT ?1 UNION SELECT tags.id FROM tags JOIN tree ON parent_id=tree.id)";
            c.execute(&format!("{tree} DELETE FROM tag_ranges WHERE tag_id IN tree"), [id])?;
            c.execute(&format!("{tree} DELETE FROM rules WHERE tag_id IN tree"), [id])?;
            if c.execute(&format!("{tree} DELETE FROM tags WHERE id IN tree"), [id])? == 0 { return Err(StorageError::NotFound); }
            Ok(())
        })
    }
}

impl Storage {
    /// Latest completed AFK run, using full UTC intervals independently of the shown day.
    pub fn completed_afk(&self) -> Result<Option<(i64, i64)>, StorageError> {
        let valid = |e: &Event, activity: &str| {
            e.data["machine"] == "awake"
                && e.data["locked"] != true
                && e.data["activity"] == activity
                && matches!(
                    e.data["recording"].as_str(),
                    Some("recording" | "unobserved")
                )
        };
        let Some(last) = self.last_event("computer")? else {
            return Ok(None);
        };
        if !valid(&last, "active") {
            return Ok(None);
        }
        let mut boundary = last.start_ts;
        let mut end = None;
        let mut query=self.connection.prepare("SELECT id,bucket,start_ts,end_ts,data FROM events WHERE bucket='computer' AND start_ts<?1 ORDER BY start_ts DESC,id DESC")?;
        let events = query.query_map([last.start_ts], event_from_row)?;
        for event in events {
            let event = event?;
            if event.end_ts != boundary {
                break;
            }
            if end.is_none() && valid(&event, "active") {
                boundary = event.start_ts;
                continue;
            }
            if !valid(&event, "away") {
                break;
            }
            end.get_or_insert(event.end_ts);
            boundary = event.start_ts;
        }
        Ok(end.map(|end| (boundary, end)))
    }
}
