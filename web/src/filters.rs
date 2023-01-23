use std::sync::Arc;

use num_format::ToFormattedString;
use progscrape_scrapers::{StoryDate, StoryDuration};
use serde_json::Value;

use super::static_files::StaticFileRegistry;

#[derive(Default)]
pub struct CommaFilter {}

impl tera::Filter for CommaFilter {
    fn filter(
        &self,
        value: &Value,
        _args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        Ok(value
            .as_i64()
            .unwrap_or_else(|| {
                tracing::warn!("Invalid input to comma filter");
                0
            })
            .to_formatted_string(&num_format::Locale::en)
            .into())
    }
}

#[derive(Default)]
pub struct AbsoluteTimeFilter {}

impl tera::Filter for AbsoluteTimeFilter {
    fn filter(
        &self,
        value: &Value,
        _args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        let date = value.as_i64().and_then(StoryDate::from_seconds);
        if let Some(date) = date {
            Ok(format!("{}", date).into())
        } else {
            Err("Invalid date arguments".to_string().into())
        }
    }
}

#[derive(Default)]
pub struct RelativeTimeFilter {}

impl tera::Filter for RelativeTimeFilter {
    fn filter(
        &self,
        value: &Value,
        args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        let date = value.as_i64().and_then(StoryDate::from_seconds);
        let now = args
            .get("now")
            .and_then(Value::as_i64)
            .and_then(StoryDate::from_seconds);
        if let (Some(date), Some(now)) = (date, now) {
            let relative = now - date;
            if relative > StoryDuration::days(60) {
                Ok(format!("{} months ago", relative.num_days() / 30).into())
            } else if relative > StoryDuration::days(2) {
                Ok(format!("{} days ago", relative.num_days()).into())
            } else if relative > StoryDuration::minutes(120) {
                Ok(format!("{} hours ago", relative.num_hours()).into())
            } else if relative > StoryDuration::minutes(60) {
                Ok("an hour ago".into())
            } else {
                Ok("recently added".into())
            }
        } else {
            Err("Invalid date arguments".to_string().into())
        }
    }
}

pub struct StaticFileFilter {
    static_files: Arc<StaticFileRegistry>,
}

impl StaticFileFilter {
    pub fn new(static_files: Arc<StaticFileRegistry>) -> Self {
        Self { static_files }
    }
}

impl tera::Filter for StaticFileFilter {
    fn filter(
        &self,
        value: &Value,
        _args: &std::collections::HashMap<String, Value>,
    ) -> tera::Result<Value> {
        let key = value.as_str().unwrap_or_else(|| {
            tracing::warn!("Invalid input to static filter");
            ""
        });
        let s = format!(
            "/static/{}",
            self.static_files.lookup_key(key).unwrap_or_else(|| {
                tracing::warn!("Static file not found: {}", key);
                "<invalid>"
            })
        );
        Ok(s.into())
    }
}
