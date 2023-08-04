use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Config {
    /// The homeserver url
    pub homeserver: String,

    /// The username of the bot
    pub username: String,

    /// The password of the bot
    pub password: String,

    /// The directory containing the stable diffusion models
    pub stable_diffusion_models: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .add_source(config::Environment::with_prefix("MATRIX_BOT"))
            .build()?;

        config.try_deserialize()
    }
}
