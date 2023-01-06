use base64::{
    alphabet::{self, Alphabet},
    engine::fast_portable::{self, FastPortable, FastPortableConfig},
};
use serde::{Deserialize, Serialize};

use crate::scrapers::{Scrape, ScrapeData, ScrapeId};
use std::{collections::HashMap, fmt::Display};

mod date;
mod url;

pub use self::{
    date::StoryDate,
    url::{StoryUrl, StoryUrlNorm},
};

/// Rendered story with all properties hydrated from the underlying scrapes. Extraneous data is removed at this point.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct StoryRender {
    pub id: String,
    pub url: String,
    pub domain: String,
    pub title: String,
    pub date: StoryDate,
    pub tags: Vec<String>,
    pub comment_links: HashMap<String, String>,
    pub scrapes: HashMap<String, Scrape>,
}

/// Uniquely identifies a story within the index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct StoryIdentifier {
    pub norm: StoryUrlNorm,
    date: (u16, u8, u8),
}

impl Display for StoryIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{}:{}:{}:{}",
            self.date.0,
            self.date.1,
            self.date.2,
            self.norm.string()
        ))
    }
}

impl StoryIdentifier {
    const BASE64_CONFIG: FastPortable =
        FastPortable::from(&alphabet::URL_SAFE, fast_portable::NO_PAD);

    pub fn new(date: StoryDate, norm: &StoryUrlNorm) -> Self {
        Self {
            norm: norm.clone(),
            date: (date.year() as u16, date.month() as u8, date.day() as u8),
        }
    }

    pub fn update_date(&mut self, date: StoryDate) {
        self.date = (date.year() as u16, date.month() as u8, date.day() as u8);
    }

    pub fn matches_date(&self, date: StoryDate) -> bool {
        (self.date.0, self.date.1, self.date.2)
            == (date.year() as u16, date.month() as u8, date.day() as u8)
    }

    pub fn to_base64(&self) -> String {
        base64::encode_engine(self.to_string().as_bytes(), &Self::BASE64_CONFIG)
    }

    pub fn from_base64<T: AsRef<[u8]>>(s: T) -> Option<Self> {
        // Use an inner function so we can make use of ? (is there an easier way?)
        fn from_base64_res<T: AsRef<[u8]>>(s: T) -> Result<StoryIdentifier, ()> {
            let s = base64::decode_engine(s, &StoryIdentifier::BASE64_CONFIG).map_err(drop)?;
            let s = String::from_utf8(s).map_err(drop)?;
            let mut bits = s.splitn(4, ':');
            let year = bits.next().ok_or(())?;
            let month = bits.next().ok_or(())?;
            let day = bits.next().ok_or(())?;
            let norm = bits.next().ok_or(())?.to_owned();
            Ok(StoryIdentifier {
                norm: StoryUrlNorm::from_string(norm),
                date: (
                    year.parse().map_err(drop)?,
                    month.parse().map_err(drop)?,
                    day.parse().map_err(drop)?,
                ),
            })
        }

        from_base64_res(s).ok()
    }

    pub fn year(&self) -> u16 {
        self.date.0
    }
    pub fn month(&self) -> u8 {
        self.date.1
    }
    pub fn day(&self) -> u8 {
        self.date.2
    }
}

/// Story scrape w/information from underlying sources.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Story {
    pub id: StoryIdentifier,
    pub scrapes: HashMap<ScrapeId, Scrape>,
}

impl Story {
    pub fn new(scrape: Scrape) -> Self {
        let id = StoryIdentifier::new(scrape.date(), scrape.url().normalization());
        let scrape_id = ScrapeId::new(scrape.source(), scrape.id());
        Self {
            id,
            scrapes: HashMap::from_iter([(scrape_id, scrape)]),
        }
    }

    pub fn merge(&mut self, scrape: Scrape) {
        // This logic can be improved when try_insert is stabilized
        // TODO: We need to merge duplicate scrapes
        let id = ScrapeId::new(scrape.source(), scrape.id());
        self.scrapes.insert(id, scrape);
        // The ID may change if the date changes
        self.id.update_date(self.date());
    }

    pub fn title(&self) -> String {
        self.scrapes
            .values()
            .next()
            .expect("Expected at least one")
            .title()
    }

    pub fn url(&self) -> StoryUrl {
        self.scrapes
            .values()
            .next()
            .expect("Expected at least one")
            .url()
    }

    /// Returns the date of this story, which is always the earliest scrape date.
    pub fn date(&self) -> StoryDate {
        self.scrapes
            .values()
            .map(|s| s.date()).min().unwrap_or_default()
    }

    pub fn render(&self) -> StoryRender {
        let scrapes = HashMap::from_iter(self.scrapes.iter().map(|(k, v)| (k.as_str(), v.clone())));
        StoryRender {
            id: self.id.to_base64(),
            url: self.url().to_string(),
            domain: self.url().host().to_owned(),
            title: self.title(),
            date: self.date(),
            tags: vec![],
            comment_links: HashMap::new(),
            scrapes,
        }
    }
}

#[cfg(test)]
mod test {
    use super::{StoryIdentifier, StoryDate, StoryUrl};

    #[test]
    fn test_story_identifier() {
        let url = StoryUrl::parse("https://google.com/?q=foo").expect("Failed to parse URL");
        let id = StoryIdentifier::new(StoryDate::now(), url.normalization());
        let base64 = id.to_base64();
        assert_eq!(id, StoryIdentifier::from_base64(base64).expect("Failed to decode ID"));
    }
}
