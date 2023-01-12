use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    html::unescape_entities, ScrapeConfigSource, ScrapeData, ScrapeDataInit, ScrapeError, ScrapeId,
    ScrapeSource, ScrapeSource2, Scraper,
};
use crate::story::{StoryDate, StoryUrl};

pub struct Reddit {}

impl ScrapeSource2 for Reddit {
    type Config = RedditConfig;
    type Scrape = RedditStory;
    type Scraper = RedditScraper;
    const TYPE: ScrapeSource = ScrapeSource::Reddit;
}

#[derive(Default, Serialize, Deserialize)]
pub struct RedditConfig {
    api: String,
    subreddit_batch: usize,
    limit: usize,
    subreddits: HashMap<String, SubredditConfig>,
}

impl ScrapeConfigSource for RedditConfig {
    fn subsources(&self) -> Vec<String> {
        self.subreddits.iter().map(|s| s.0.clone()).collect()
    }

    fn provide_urls(&self, subsources: Vec<String>) -> Vec<String> {
        let mut output = vec![];
        for chunk in subsources.chunks(self.subreddit_batch) {
            output.push(
                self.api.replace("${subreddits}", &chunk.join("+"))
                    + &format!("?limit={}", self.limit),
            )
        }
        output
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct SubredditConfig {
    #[serde(default)]
    is_tag: bool,
    #[serde(default)]
    flair_is_tag: bool,
}

#[derive(Default)]
pub struct RedditScraper {}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RedditStory {
    pub title: String,
    pub url: StoryUrl,
    pub subreddit: String,
    pub flair: String,
    pub id: String,
    pub position: u32,
    pub upvotes: u32,
    pub downvotes: u32,
    pub num_comments: u32,
    pub score: u32,
    pub upvote_ratio: f32,
    pub date: StoryDate,
}

impl ScrapeData for RedditStory {
    fn title(&self) -> String {
        self.title.clone()
    }

    fn url(&self) -> StoryUrl {
        self.url.clone()
    }

    fn comments_url(&self) -> String {
        unimplemented!()
    }

    fn source(&self) -> ScrapeId {
        ScrapeId::new(
            ScrapeSource::Reddit,
            Some(self.subreddit.clone()),
            self.id.clone(),
        )
    }

    fn date(&self) -> StoryDate {
        self.date
    }
}

impl ScrapeDataInit<RedditStory> for RedditStory {
    fn initialize_required(
        id: String,
        title: String,
        url: StoryUrl,
        date: StoryDate,
    ) -> RedditStory {
        Self {
            title,
            url,
            id,
            date,
            subreddit: Default::default(),
            flair: Default::default(),
            position: Default::default(),
            upvotes: Default::default(),
            downvotes: Default::default(),
            num_comments: Default::default(),
            score: Default::default(),
            upvote_ratio: Default::default(),
        }
    }

    fn merge(&mut self, other: RedditStory) {
        self.title = other.title;
        self.url = other.url;
        self.date = std::cmp::min(self.date, other.date);
        self.flair = other.flair;
        self.position = std::cmp::max(self.position, other.position);
        self.upvotes = std::cmp::max(self.upvotes, other.upvotes);
        self.downvotes = std::cmp::max(self.downvotes, other.downvotes);
        self.num_comments = std::cmp::max(self.num_comments, other.num_comments);
        self.score = std::cmp::max(self.score, other.score);
        self.upvote_ratio = f32::max(self.upvote_ratio, other.upvote_ratio);
    }
}

impl RedditScraper {
    fn require_string(&self, data: &Value, key: &str) -> Result<String, String> {
        Ok(data[key]
            .as_str()
            .ok_or(format!("Missing field {:?}", key))?
            .to_owned())
    }

    fn optional_string(&self, data: &Value, key: &str) -> Result<String, String> {
        Ok(data[key].as_str().unwrap_or_default().to_owned())
    }

    fn require_integer<T: TryFrom<i64> + TryFrom<u64>>(
        &self,
        data: &Value,
        key: &str,
    ) -> Result<T, String> {
        if let Value::Number(n) = &data[key] {
            if let Some(n) = n.as_u64() {
                if let Ok(n) = n.try_into() {
                    return Ok(n);
                }
            }
            if let Some(n) = n.as_i64() {
                if let Ok(n) = n.try_into() {
                    return Ok(n);
                }
            }
            if let Some(n) = n.as_f64() {
                let n = n as i64;
                if let Ok(n) = n.try_into() {
                    return Ok(n);
                }
            }
            Err(format!(
                "Failed to parse {} as integer (value was {:?})",
                key, n
            ))
        } else {
            Err(format!(
                "Missing or invalid field {:?} (value was {:?})",
                key, data[key]
            ))
        }
    }

    fn require_float(&self, data: &Value, key: &str) -> Result<f64, String> {
        if let Value::Number(n) = &data[key] {
            if let Some(n) = n.as_u64() {
                return Ok(n as f64);
            }
            if let Some(n) = n.as_i64() {
                return Ok(n as f64);
            }
            if let Some(n) = n.as_f64() {
                return Ok(n);
            }
            Err(format!(
                "Failed to parse {} as float (value was {:?})",
                key, n
            ))
        } else {
            Err(format!(
                "Missing or invalid field {:?} (value was {:?})",
                key, data[key]
            ))
        }
    }

    fn map_story(&self, child: &Value, position: u32) -> Result<RedditStory, String> {
        let kind = child["kind"].as_str();
        let data = if kind == Some("t3") {
            &child["data"]
        } else {
            return Err(format!("Unknown story type: {:?}", kind));
        };

        let millis = self.require_integer(data, "created_utc")?;
        let date = StoryDate::from_millis(millis).ok_or_else(|| "Unmappable date".to_string())?;
        let url = StoryUrl::parse(unescape_entities(&self.require_string(data, "url")?))
            .ok_or_else(|| "Unmappable URL".to_string())?;
        let story = RedditStory {
            title: unescape_entities(&self.require_string(data, "title")?),
            url,
            num_comments: self.require_integer(data, "num_comments")?,
            score: self.require_integer(data, "score")?,
            downvotes: self.require_integer(data, "downs")?,
            upvotes: self.require_integer(data, "ups")?,
            upvote_ratio: self.require_float(data, "upvote_ratio")? as f32,
            subreddit: self.require_string(data, "subreddit")?,
            flair: unescape_entities(&self.optional_string(data, "link_flair_text")?),
            id: self.require_string(data, "id")?,
            date,
            position,
        };
        Ok(story)
    }
}

impl Scraper<RedditConfig, RedditStory> for RedditScraper {
    fn scrape(
        &self,
        _args: &RedditConfig,
        input: String,
    ) -> Result<(Vec<RedditStory>, Vec<String>), ScrapeError> {
        let root: Value = serde_json::from_str(&input)?;
        let mut value = &root;
        for path in ["data", "children"] {
            if let Some(object) = value.as_object() {
                if let Some(nested_value) = object.get(path) {
                    value = nested_value;
                } else {
                    return Err(ScrapeError::StructureError(
                        "Failed to parse Reddit JSON data.children".to_owned(),
                    ));
                }
            }
        }

        if let Some(children) = value.as_array() {
            let mut vec = vec![];
            let mut errors = vec![];
            for (position, child) in children.iter().enumerate() {
                match self.map_story(child, position as u32) {
                    Ok(story) => vec.push(story),
                    Err(e) => errors.push(e),
                }
            }
            Ok((vec, errors))
        } else {
            Err(ScrapeError::StructureError(
                "Missing children element".to_owned(),
            ))
        }
    }

    fn provide_tags(
        &self,
        args: &RedditConfig,
        scrape: &RedditStory,
        tags: &mut crate::story::TagSet) -> Result<(), super::ScrapeError> {
    if let Some(subreddit) = args.subreddits.get(&scrape.subreddit) {
        if subreddit.flair_is_tag {
            tags.add(&scrape.flair);
        }
        if subreddit.is_tag {
            tags.add(&scrape.subreddit);
        }
    }
    Ok(())
}
}

#[cfg(test)]
pub mod test {
    use super::super::test::*;
    use super::*;

    pub fn scrape_all() -> Vec<RedditStory> {
        let mut all = vec![];
        let scraper = RedditScraper::default();
        for file in reddit_files() {
            let stories = scraper
                .scrape(&RedditConfig::default(), load_file(file))
                .unwrap_or_else(|_| panic!("Failed to parse a story from {}", file));
            all.extend(stories.0);
        }
        all
    }

    #[test]
    fn test_parse_sample() {
        let scraper = RedditScraper::default();
        for file in reddit_files() {
            let stories = scraper
                .scrape(&RedditConfig::default(), load_file(file))
                .unwrap();
            for story in stories.0 {
                println!("[{}] {} ({})", story.subreddit, story.title, story.url);
            }
        }
    }
}
