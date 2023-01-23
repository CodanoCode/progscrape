mod persist;
mod story;

pub use persist::{
    MemIndex, PersistError, PersistLocation, Storage, StorageSummary, StorageWriter, StoryIndex,
};
pub use story::{
    Story, StoryEvaluator, StoryIdentifier, StoryRender, StoryScoreConfig, TaggerConfig,
};

#[cfg(test)]
mod test {
    use rstest::*;
    use tracing_subscriber::EnvFilter;

    #[fixture]
    #[once]
    pub fn enable_tracing() -> bool {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .init();
        true
    }

    #[fixture]
    #[once]
    pub fn enable_slow_tests() -> bool {
        matches!(std::env::var("ENABLE_SLOW_TESTS"), Ok(_))
    }
}
