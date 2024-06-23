use std::{collections::HashMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum ConfigError {
    IoError(io::Error),
    InvalidConfig(toml::de::Error),
}
// These implementations allow us to use the `?` operator on functions that
// don't necessarily return ConfigError.
impl From<io::Error> for ConfigError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(value: toml::de::Error) -> Self {
        Self::InvalidConfig(value)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AppConfig {
    pub grpcport: String,
    pub username: String,
    pub password: String,
    pub baseurl: String,
    pub urlget: String,
    pub urlfilter: Vec<(String, Vec<String>)>,
    pub entries: u32,
    pub filter1: String,
    pub filter2: String,
    pub urlput: String,
    pub printmode: bool,
    pub checkmode: bool,
    pub filemode: bool,
}

/* #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Urlfilter {
    urlfilter: HashMap<String, Vec<String>>,
} */

impl Default for AppConfig {
    fn default() -> Self {
        let urlfilter = vec![(
            "aa::bb::cc".to_string(),
            vec!["AA".to_string(), "BB".to_string()],
        )];

        Self {
            grpcport: "8001".to_string(),
            username: "testuser".to_string(),
            password: "testPW".to_string(),
            baseurl: "http://localhost:8000".to_string(),
            urlget: "/te?q=123+eq+123 AND ".to_string(),
            urlfilter,
            entries: 10,
            filter1: "AAccount".to_string(),
            filter2: "Update".to_string(),
            urlput: "/users".to_string(),
            printmode: true,
            checkmode: false,
            filemode: false,
        }
    }
}
pub fn load_or_initialize(filename: &str) -> Result<AppConfig, ConfigError> {
    let config_path = Path::new(filename);
    if config_path.exists() {
        // The `?` operator tells Rust, if the value is an error, return that error.
        // You can also use the `?` operator on the Option enum.

        let content = fs::read_to_string(config_path)?;
        let config = toml::from_str(&content)?;

        return Ok(config);
    }

    // The config file does not exist, so we must initialize it with the default values.
    let config = AppConfig::default();
    let toml = toml::to_string(&config).unwrap();

    fs::write(config_path, toml)?;
    Ok(config)
}

pub fn confload(file: &str) -> Result<AppConfig, ConfigError> {
    let config: AppConfig = match load_or_initialize(file) {
        Ok(v) => v,
        Err(err) => {
            /* match err {
                ConfigError::IoError(err) => {
                    eprintln!("An error occurred while loading the config: {err}");
                }
                ConfigError::InvalidConfig(err) => {
                    eprintln!("An error occurred while parsing the config:");
                    eprintln!("{err}");
                }
            } */
            return Err(err);
        }
    };

    Ok(config)
    //println!("{:?}", config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parse() -> Result<(), Box<dyn std::error::Error>> {
        let filename1 = "Config.toml";
        let file = assert_fs::NamedTempFile::new("Config.toml")?;
        //file.write_str("A test\nActual content\nMore content\nAnother test")?;
        println!("{:?}", file.path());
        //let filename = "Config.toml";
        let conf = load_or_initialize(filename1).unwrap();
        //findReplace(hay, r"^ki");
        //let result = 2 + 2;
        let o = AppConfig::default();
        println!("{:?}", conf);
        assert_eq!(conf, o);
        Ok(())
    }
}
