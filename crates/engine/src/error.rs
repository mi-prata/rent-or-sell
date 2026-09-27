use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("invalid input: {0}")]
    Invalid(String),
    // Not `#[from]`/`#[source]`: the message already includes the parser's
    // text, so exposing it as a source would print it twice in error chains.
    #[error("could not parse TOML: {0}")]
    Toml(toml::de::Error),
}

impl From<toml::de::Error> for EngineError {
    fn from(e: toml::de::Error) -> Self {
        EngineError::Toml(e)
    }
}

pub(crate) fn invalid(msg: impl Into<String>) -> EngineError {
    EngineError::Invalid(msg.into())
}
