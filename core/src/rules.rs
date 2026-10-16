use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: i64,
    pub field: String,
    pub pattern: String,
    pub tag_id: i64,
    pub priority: i64,
    pub enabled: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct RuleMatch {
    pub rule_id: i64,
    pub tag_id: i64,
    pub start_ts: i64,
    pub end_ts: i64,
    pub sample: String,
}
pub(crate) fn compile(pattern: &str) -> Result<regex::Regex, StorageError> {
    regex::RegexBuilder::new(pattern)
        .size_limit(1_000_000)
        .build()
        .map_err(|_| {
            StorageError::InvalidRecord("Invalid or too complex regular expression".into())
        })
}
impl Storage {
    pub fn rules(&self) -> Result<Vec<Rule>, StorageError> {
        let mut query = self.connection.prepare(
            "SELECT id,field,pattern,tag_id,priority,enabled FROM rules ORDER BY priority DESC,id",
        )?;
        let result = query
            .query_map([], |r| {
                Ok(Rule {
                    id: r.get(0)?,
                    field: r.get(1)?,
                    pattern: r.get(2)?,
                    tag_id: r.get(3)?,
                    priority: r.get(4)?,
                    enabled: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(result)
    }
    /// Partition observed active time; manual and already applied tags always win.
    pub fn preview_rules(
        &self,
        from: i64,
        to: i64,
        draft: Option<&Rule>,
    ) -> Result<Vec<RuleMatch>, StorageError> {
        if from < 0 || to <= from {
            return Err(StorageError::InvalidRecord(
                "expected 0 <= from < to".into(),
            ));
        }
        let mut rules = self.rules()?;
        if let Some(draft) = draft {
            if !["app", "title", "url"].contains(&draft.field.as_str()) || draft.tag_id <= 0 {
                return Err(StorageError::InvalidRecord(
                    "invalid rule field, pattern or priority".into(),
                ));
            }
            compile(&draft.pattern)?;
            rules.retain(|r| r.id != draft.id);
            rules.push(draft.clone());
            rules.sort_by_key(|r| (std::cmp::Reverse(r.priority), r.id));
        }
        let tags = self.tags()?;
        let rules = rules
            .into_iter()
            .filter(|r| r.enabled && tags.iter().any(|t| t.id == r.tag_id && t.archived == 0))
            .map(|r| Ok((compile(&r.pattern)?, r)))
            .collect::<Result<Vec<_>, StorageError>>()?;
        if rules.is_empty() {
            return Ok(Vec::new());
        }
        // shortcut: previews share the 100,000-event query cap; chunk history when larger ranges are needed.
        let events = self.events(None, from, to)?;
        let ranges = self.tag_ranges(from, to)?;
        let mut bounds = vec![from, to];
        for event in &events {
            bounds.extend([event.start_ts.max(from), event.end_ts.min(to)]);
        }
        for range in &ranges {
            bounds.extend([range.start_ts.max(from), range.end_ts.min(to)]);
        }
        bounds.sort_unstable();
        bounds.dedup();
        let mut result: Vec<RuleMatch> = Vec::new();
        // Bucket observations are nonoverlapping; move cursors instead of rescanning history.
        let bucket = |name: &str| {
            events
                .iter()
                .filter(|e| e.bucket == name)
                .collect::<Vec<_>>()
        };
        let (computer, afk, windows, web) = (
            bucket("computer"),
            bucket("afk"),
            bucket("window"),
            bucket("web"),
        );
        let (mut ci, mut ai, mut wi, mut bi) = (0, 0, 0, 0);
        let (mut range_index, mut covered_until) = (0, from);
        for pair in bounds.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let c = at(&computer, &mut ci, start, end);
            let a = at(&afk, &mut ai, start, end);
            let w = at(&windows, &mut wi, start, end);
            let b = at(&web, &mut bi, start, end);
            let active = super::reports::observed_active(c, a, computer.is_empty());
            while range_index < ranges.len() && ranges[range_index].start_ts <= start {
                covered_until = covered_until.max(ranges[range_index].end_ts);
                range_index += 1;
            }
            if !active || start < covered_until {
                continue;
            }
            for (regex, rule) in &rules {
                let event = if rule.field == "url" { b } else { w.or(b) };
                let Some(value) = event.and_then(|e| e.data[&rule.field].as_str()) else {
                    continue;
                };
                if regex.is_match(value) {
                    if let Some(last) = result.last_mut().filter(|r| {
                        r.rule_id == rule.id && r.tag_id == rule.tag_id && r.end_ts == start
                    }) {
                        last.end_ts = end;
                    } else {
                        result.push(RuleMatch {
                            rule_id: rule.id,
                            tag_id: rule.tag_id,
                            start_ts: start,
                            end_ts: end,
                            sample: value.chars().take(160).collect(),
                        });
                    }
                    break;
                }
            }
        }
        Ok(result)
    }
    /// Recompute under the write transaction so a preview can never overwrite a newer edit.
    pub fn apply_rules(
        &mut self,
        from: i64,
        to: i64,
    ) -> Result<(Vec<RuleMatch>, TagUndo), StorageError> {
        self.apply_rules_inner(from, to, None)
    }
    pub fn apply_rules_checked(
        &mut self,
        from: i64,
        to: i64,
        expected: &[RuleMatch],
    ) -> Result<(Vec<RuleMatch>, TagUndo), StorageError> {
        self.apply_rules_inner(from, to, Some(expected))
    }
    fn apply_rules_inner(
        &mut self,
        from: i64,
        to: i64,
        expected: Option<&[RuleMatch]>,
    ) -> Result<(Vec<RuleMatch>, TagUndo), StorageError> {
        self.connection.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let preview = self.preview_rules(from, to, None)?;
            if expected.is_some_and(|expected| expected != preview) {
                return Err(StorageError::InvalidRecord(
                    "Tags changed since preview; preview again".into(),
                ));
            }
            let mut inserted = Vec::new();
            for r in &preview {
                self.connection.execute(
                    "INSERT INTO tag_ranges(tag_id,start_ts,end_ts) VALUES(?1,?2,?3)",
                    params![r.tag_id, r.start_ts, r.end_ts],
                )?;
                inserted.push(TagRange {
                    precedence_id: None,
                    id: self.connection.last_insert_rowid(),
                    tag_id: r.tag_id,
                    start_ts: r.start_ts,
                    end_ts: r.end_ts,
                    note: None,
                });
            }
            Ok((preview, TagUndo::inserted(inserted)))
        })();
        match result {
            Ok(result) => {
                if let Err(error) = self.connection.execute_batch("COMMIT") {
                    self.connection.execute_batch("ROLLBACK")?;
                    return Err(error.into());
                }
                self.metadata_version = self.metadata_version.wrapping_add(1);
                Ok(result)
            }
            Err(error) => {
                self.connection.execute_batch("ROLLBACK")?;
                Err(error)
            }
        }
    }
    /// Apply only newly observed time, retaining existing manual tags without creating undo snapshots.
    pub fn auto_tag(&mut self, from: i64, to: i64) -> Result<(), StorageError> {
        if to <= from {
            return Ok(());
        }
        let matches = self.preview_rules(from, to, None)?;
        if matches.is_empty() {
            return Ok(());
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let next_version = self.metadata_version.wrapping_add(1);
        let mut tail = self.auto_tail;
        for r in matches {
            let merged = if let Some((rule, id, end, revision)) = tail {
                rule==r.rule_id && end==r.start_ts && revision==self.metadata_version
                    && tx.execute("UPDATE tag_ranges SET end_ts=?2 WHERE id=?1 AND tag_id=?3 AND end_ts=?4 AND note IS NULL",params![id,r.end_ts,r.tag_id,end])?>0
            } else {
                false
            };
            let id = if merged {
                tail.unwrap().1
            } else {
                if tx.execute("INSERT INTO tag_ranges(tag_id,start_ts,end_ts) SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM tag_ranges WHERE start_ts<?3 AND end_ts>?2)",params![r.tag_id,r.start_ts,r.end_ts])?==0 { continue; }
                tx.last_insert_rowid()
            };
            tail = Some((r.rule_id, id, r.end_ts, self.metadata_version));
        }
        tx.commit()?;
        self.auto_tail = tail.map(|(rule, id, end, _)| (rule, id, end, next_version));
        self.metadata_version = next_version;
        Ok(())
    }
}

fn at<'a>(items: &[&'a Event], index: &mut usize, start: i64, end: i64) -> Option<&'a Event> {
    while *index < items.len() && items[*index].end_ts <= start {
        *index += 1;
    }
    items
        .get(*index)
        .copied()
        .filter(|e| e.start_ts <= start && e.end_ts >= end)
}
