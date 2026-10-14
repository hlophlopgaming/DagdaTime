use super::*;
use draugr_time_core::{Collection, Rule, RuleMatch};
use iced::widget::column;
use serde_json::json;

#[derive(Default)]
pub struct Editor {
    pub id: Option<i64>,
    pub field: String,
    pub pattern: String,
    pub tag_id: Option<i64>,
    pub priority: String,
    pub enabled: bool,
    pub from: String,
    pub to: String,
    pub preview: Vec<RuleMatch>,
    pub error: String,
    pub confirm: bool,
}
#[derive(Clone, Debug)]
pub enum Edit {
    New,
    Select(i64),
    Field(String),
    Pattern(String),
    Tag(i64),
    Priority(String),
    Enabled(bool),
    From(String),
    To(String),
    Save,
    Delete,
    Preview,
    Apply,
    Confirm,
    Cancel,
}
impl Editor {
    fn draft(&self) -> Result<Rule, String> {
        Ok(Rule {
            id: self.id.unwrap_or(0),
            field: self.field.clone(),
            pattern: self.pattern.clone(),
            tag_id: self.tag_id.ok_or_else(|| tr("Choose a tag"))?,
            priority: self
                .priority
                .parse()
                .map_err(|_| tr("Priority must be an integer"))?,
            enabled: self.enabled,
        })
    }
    fn bounds(&self) -> Result<Range, String> {
        let parse = |s: &str| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| tr("Use YYYY-MM-DD dates"))
        };
        let range = Range::new(
            model::day_bounds(parse(&self.from)?).start,
            model::day_bounds(parse(&self.to)?).end,
        );
        if range.start < 0 || range.end <= range.start {
            return Err(tr("Choose a valid date range"));
        }
        Ok(range)
    }
}
impl State {
    pub fn edit_rule(&mut self, event: Edit) {
        let Some(runtime) = &self.runtime else {
            self.error = tr("Start recording to edit rules");
            return;
        };
        let mut db = runtime.shared.storage.lock().unwrap();
        let e = &mut self.rule_editor;
        e.confirm = false;
        match event {
            Edit::New => {
                *e = Editor {
                    field: "app".into(),
                    priority: "0".into(),
                    enabled: true,
                    from: self
                        .date
                        .checked_sub_days(Days::new(6))
                        .unwrap_or(self.date)
                        .to_string(),
                    to: self.date.to_string(),
                    ..Default::default()
                };
            }
            Edit::Select(id) => match db.rules() {
                Ok(rules) => {
                    if let Some(r) = rules.into_iter().find(|r| r.id == id) {
                        *e = Editor {
                            id: Some(id),
                            field: r.field,
                            pattern: r.pattern,
                            tag_id: Some(r.tag_id),
                            priority: r.priority.to_string(),
                            enabled: r.enabled,
                            from: self
                                .date
                                .checked_sub_days(Days::new(6))
                                .unwrap_or(self.date)
                                .to_string(),
                            to: self.date.to_string(),
                            ..Default::default()
                        };
                    }
                }
                Err(error) => e.error = error.to_string(),
            },
            Edit::Field(value) => e.field = value,
            Edit::Pattern(value) => e.pattern = value,
            Edit::Tag(id) => e.tag_id = Some(id),
            Edit::Priority(value) => e.priority = value,
            Edit::Enabled(value) => e.enabled = value,
            Edit::From(value) => e.from = value,
            Edit::To(value) => e.to = value,
            Edit::Save => {
                let result=e.draft().and_then(|r|db.put(Collection::Rules,e.id,json!({"field":r.field,"pattern":r.pattern,"tag_id":r.tag_id,"priority":r.priority,"enabled":r.enabled})).map_err(|e|e.to_string()));
                match result {
                    Ok(value) => {
                        e.id = value["id"].as_i64();
                        self.error.clear();
                    }
                    Err(error) => {
                        e.error = error;
                        return;
                    }
                }
            }
            Edit::Delete => {
                if let Some(id) = e.id {
                    match db.delete(Collection::Rules, id) {
                        Ok(_) => *e = Editor::default(),
                        Err(error) => e.error = error.to_string(),
                    }
                }
                return;
            }
            Edit::Apply => {
                match e.bounds().and_then(|range| {
                    db.preview_rules(range.start, range.end, None)
                        .map_err(|e| e.to_string())
                }) {
                    Ok(preview) => {
                        e.preview = preview;
                        e.confirm = true;
                        e.error.clear();
                    }
                    Err(error) => e.error = error,
                }
                return;
            }
            Edit::Confirm => {
                let result = e.bounds().and_then(|range| {
                    db.apply_rules_checked(range.start, range.end, &e.preview)
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok((_, undo)) => {
                        if self.tag_undo.len() == 20 {
                            self.tag_undo.remove(0);
                        }
                        self.tag_undo.push(undo);
                        e.preview.clear();
                        e.error.clear();
                    }
                    Err(error) => e.error = error,
                }
                drop(db);
                self.invalidate_load();
                return;
            }
            Edit::Cancel => return,
            Edit::Preview => {}
        }
        match e.bounds().and_then(|range| {
            e.draft().and_then(|draft| {
                db.preview_rules(range.start, range.end, Some(&draft))
                    .map_err(|e| e.to_string())
            })
        }) {
            Ok(preview) => {
                e.preview = preview;
                e.error.clear();
            }
            Err(error) => {
                e.preview.clear();
                e.error = error;
            }
        }
    }
    pub fn rules_view(&self) -> Element<'_, Message> {
        let Some(runtime) = &self.runtime else {
            return column![
                text(tr("Start recording to edit rules")),
                button(text(tr("Start recording"))).on_press(Message::Start)
            ]
            .spacing(10)
            .into();
        };
        let db = runtime.shared.storage.lock().unwrap();
        let rules = match db.rules() {
            Ok(r) => r,
            Err(e) => return text(e.to_string()).into(),
        };
        let tags = db.tags().unwrap_or_default();
        drop(db);
        let e = &self.rule_editor;
        let mut list = column![
            text(tr("Auto-tagging rules")).size(22),
            text(tr(
                "Higher priority wins; ties use the oldest rule. Existing tags are preserved."
            )),
            button(text(tr("New rule"))).on_press(Message::Rule(Edit::New))
        ]
        .spacing(8);
        for rule in rules {
            list = list.push(
                button(text(format!(
                    "{} · {} · {} → {} · {}",
                    if rule.enabled { tr("On") } else { tr("Off") },
                    rule.priority,
                    tr(&rule.field),
                    model::tag_path(&tags, rule.tag_id),
                    rule.pattern
                )))
                .on_press(Message::Rule(Edit::Select(rule.id))),
            );
        }
        if e.field.is_empty() {
            return scrollable(list).into();
        }
        let mut fields = row![].spacing(6);
        for field in ["app", "title", "url"] {
            fields = fields.push(
                button(text(tr(field)))
                    .on_press(Message::Rule(Edit::Field(field.into())))
                    .style(if e.field == field {
                        button::primary
                    } else {
                        button::secondary
                    }),
            );
        }
        let mut choices = column![text(tr("Choose a tag"))].spacing(4);
        for tag in tags.iter().filter(|t| t.archived == 0) {
            choices = choices.push(
                button(text(model::tag_path(&tags, tag.id)))
                    .on_press(Message::Rule(Edit::Tag(tag.id)))
                    .style(if e.tag_id == Some(tag.id) {
                        button::primary
                    } else {
                        button::secondary
                    }),
            );
        }
        let total = e.preview.iter().map(|r| r.end_ts - r.start_ts).sum::<i64>();
        let mut form = column![
            fields,
            text_input(&tr("Regular expression"), &e.pattern)
                .on_input(|v| Message::Rule(Edit::Pattern(v))),
            text_input(&tr("Priority"), &e.priority).on_input(|v| Message::Rule(Edit::Priority(v))),
            checkbox(e.enabled)
                .label(tr("Enabled"))
                .on_toggle(|v| Message::Rule(Edit::Enabled(v))),
            scrollable(choices).height(140),
            row![
                button(text(tr("Save rule"))).on_press(Message::Rule(Edit::Save)),
                button(text(tr("Delete rule")))
                    .on_press_maybe(e.id.map(|_| Message::Rule(Edit::Delete)))
            ]
            .spacing(6),
            text(tr("History date range (inclusive)")),
            row![
                text_input("YYYY-MM-DD", &e.from).on_input(|v| Message::Rule(Edit::From(v))),
                text_input("YYYY-MM-DD", &e.to).on_input(|v| Message::Rule(Edit::To(v)))
            ]
            .spacing(6),
            text(localized(
                "Preview: {duration} · {count} intervals",
                &[
                    ("duration", model::duration(total)),
                    ("count", e.preview.len().to_string())
                ]
            )),
            text(tr(
                "Live preview includes the draft. Apply to history uses saved enabled rules."
            )),
            row![
                button(text(tr("Refresh preview"))).on_press(Message::Rule(Edit::Preview)),
                button(text(tr("Apply to history"))).on_press(Message::Rule(Edit::Apply))
            ]
            .spacing(6),
            text(localization::error(&e.error)).color(Color::from_rgb8(200, 75, 75))
        ]
        .spacing(8);
        for sample in e.preview.iter().take(5) {
            form = form.push(text(format!(
                "{}–{} · {} · {}",
                model::time_label(sample.start_ts),
                model::time_label(sample.end_ts),
                model::duration(sample.end_ts - sample.start_ts),
                sample.sample
            )));
        }
        if e.confirm {
            form=form.push(text(localized("Apply {duration} of automatic tags? Existing tags stay unchanged. You can undo this action.",&[("duration",model::duration(total))]))).push(row![button(text(tr("Confirm application"))).on_press(Message::Rule(Edit::Confirm)),button(text(tr("Cancel"))).on_press(Message::Rule(Edit::Cancel))].spacing(6));
        }
        scrollable(column![list, container(form).padding(12).style(panel)].spacing(16)).into()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_validates_priority_tag_and_dates() {
        let mut e = Editor {
            field: "app".into(),
            pattern: "editor".into(),
            priority: "x".into(),
            from: "2026-10-01".into(),
            to: "2026-10-07".into(),
            ..Default::default()
        };
        assert!(e.draft().is_err());
        e.tag_id = Some(1);
        e.priority = "2".into();
        assert_eq!(e.draft().unwrap().priority, 2);
        assert_eq!(e.bounds().unwrap().duration(), 7 * 86_400_000);
        e.to = "bad".into();
        assert!(e.bounds().is_err());
    }
}

#[cfg(test)]
mod workflow_tests {
    use super::*;
    #[test]
    fn editor_live_preview_confirm_apply_and_undo_match() {
        let (mut state, _) = State::boot();
        let shared = draugr_time_daemon::Shared::new(
            draugr_time_core::Storage::open_in_memory().unwrap(),
            "test".into(),
            300,
        );
        let start = state.day.start;
        {
            let mut db = shared.storage.lock().unwrap();
            db.put(Collection::Tags, None, json!({"name":"Work"}))
                .unwrap();
            db.record_interval(
                "window",
                &json!({"app":"editor","title":"Work"}),
                start,
                start + 120_000,
            )
            .unwrap();
            db.record_interval(
                "computer",
                &json!({"machine":"awake","activity":"active","recording":"recording"}),
                start,
                start + 120_000,
            )
            .unwrap();
        }
        state.runtime = Some(Runtime::for_test(shared.clone()));
        state.edit_rule(Edit::New);
        state.edit_rule(Edit::Tag(1));
        state.edit_rule(Edit::Pattern("^editor$".into()));
        assert_eq!(
            state
                .rule_editor
                .preview
                .iter()
                .map(|r| r.end_ts - r.start_ts)
                .sum::<i64>(),
            120_000
        );
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .list(Collection::Rules)
            .unwrap()
            .is_empty());
        state.edit_rule(Edit::Save);
        state.edit_rule(Edit::Apply);
        assert!(state.rule_editor.confirm);
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .tag_ranges(start, start + 200_000)
            .unwrap()
            .is_empty());
        state.edit_rule(Edit::Confirm);
        assert_eq!(
            shared
                .storage
                .lock()
                .unwrap()
                .tag_ranges(start, start + 200_000)
                .unwrap()[0]
                .end_ts,
            start + 120_000
        );
        let _ = state.edit(tagging::Edit::Undo);
        assert!(shared
            .storage
            .lock()
            .unwrap()
            .tag_ranges(start, start + 200_000)
            .unwrap()
            .is_empty());
    }
}
