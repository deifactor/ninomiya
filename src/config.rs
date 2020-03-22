use anyhow::{anyhow, Error};
use log::info;
use serde::{Deserialize, Deserializer};
use std::path::{Path, PathBuf};
use std::time::Duration;

// A custom deserializer that just deserializes an f32. We do this because the default serde
// implementation uses a {seconds, nanoseconds} tuple, which is good for exactness but bad for
// configuration.
fn deserialize_duration<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
    Ok(Duration::from_secs_f32(f32::deserialize(deserializer)?))
}

/// Configures how the GUI is rendered.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Width of notification windows.
    pub width: i32,
    /// Height of notification windows.
    pub height: i32,
    /// Amount of seconds to show windows before closing them.
    #[serde(deserialize_with = "deserialize_duration")]
    pub duration: Duration,
    /// How much verticla space to put between notifications.
    pub notification_spacing: i32,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            width: 300,
            height: 100,
            duration: Duration::from_millis(3000),
            notification_spacing: 10,
        }
    }
}

impl Config {
    /// Loads the configuration file from the on-disk config path.
    ///
    /// This uses the OS-appropriate path; for example, ~/.config on Linux.
    pub fn load() -> Result<Config, Error> {
        Config::load_from(Config::config_dir()?.join("config.toml"))
    }

    /// Loads the configuration file from the given path.
    pub fn load_from<P: AsRef<Path>>(path: P) -> Result<Config, Error> {
        let path = path.as_ref().to_str().ok_or(anyhow!(
            "Failed to convert path '{:?}' to Unicode",
            path.as_ref().to_string_lossy()
        ))?;
        info!("Attempting to load config from {}", path);
        let file = config::File::new(path, config::FileFormat::Toml);
        let mut config = config::Config::new();
        config.merge(file)?;
        let config = config.try_into()?;
        Ok(config)
    }

    /// The directory that all the configuration files are stored in.
    pub fn config_dir() -> Result<PathBuf, Error> {
        Ok(
            directories::ProjectDirs::from("ai", "deifactor", "ninomiya")
                .ok_or(anyhow!("Failed to compute config directory path"))?
                .config_dir()
                .to_owned(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Write;

    #[test]
    fn empty_config() {
        config::Config::new()
            .try_into::<Config>()
            .expect("constructing a config from an empty file should work");
    }

    #[test]
    fn nonexistent_config_path() {
        assert!(Config::load_from("/i/do/not/exist").is_err());
    }

    #[test]
    fn config_file_does_not_parse() -> Result<(), Error> {
        let mut tempfile = tempfile::NamedTempFile::new()?;
        tempfile.write_all(b"asldkfjaldskfj'!@#")?;
        assert!(Config::load_from(tempfile.path()).is_err());
        Ok(())
    }
}