use chrono::NaiveTime;
use expanduser::expanduser;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::{env, error::Error, path::PathBuf};
use toml::Table;

#[derive(Debug, Clone)]
pub struct Config {
    pub debugmode: bool,
    pub interactive: bool,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
    pub bot_token: Option<String>,
    pub persistent: bool,
    pub update_time: String,
    pub log_level: log::LevelFilter,
    pub log_path: PathBuf,
    pub admin: Option<u64>,
}
impl Default for Config {
    fn default() -> Self {
        Config {
            debugmode: false,
            interactive: false,
            config_path: expanduser("~/.config/rssnotify/config.toml").unwrap(),
            db_path: expanduser("~/.config/rssnotify/database.db3").unwrap(),
            log_path: expanduser("~/.config/rssnotify/rssnotify.log").unwrap(),
            bot_token: None,
            persistent: true,
            update_time: parse_update_time("9-17").unwrap(),
            log_level: log::LevelFilter::Info,
            admin: None,
        }
    }
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, Box<dyn Error>> {
        let bot_token = env::var("TELOXIDE_TOKEN").ok();
        let mut argstruct: Config = Config {
            bot_token,
            ..Default::default()
        };
        argstruct.apply_args(args)?;

        #[cfg(debug_assertions)]
        println!("Debug version: setting filepaths to target/debug");
        #[cfg(debug_assertions)]
        argstruct.set_paths_debug_mode();

        Ok(argstruct)
    }

    #[cfg(debug_assertions)]
    fn set_paths_debug_mode(&mut self) {
        self.log_path = expanduser("target/debug/rssnotify.log").unwrap();
        self.config_path = expanduser("rssnotify.toml").unwrap();
        self.db_path = expanduser("target/debug/database.db3").unwrap();
    }

    // TODO
    pub fn build_from_toml_and_args(args: &[String]) -> Result<Config, Box<dyn Error>> {
        let mut config = Config::build(args)?;
        if config.config_path.is_file() {
            config.apply_toml(config.config_path.clone().as_path())?;
            config.apply_args(args)?; // Overwrite with arguments
        }
        Ok(config)
    }

    pub fn apply_toml(&mut self, file_path: &Path) -> Result<(), Box<dyn Error>> {
        let content = fs::read_to_string(file_path)?;
        let table = content.parse::<Table>()?;
        let table = table["config"]
            .as_table()
            .ok_or("File does not contain a [config] header!")?;
        for key in table.keys() {
            match key.as_str() {
                // TODO
                "admin" => {
                    self.admin = table["admin"]
                        .as_integer()
                        .map(|int| int.try_into().unwrap())
                }
                "bot_token" => match table["bot_token"].as_str() {
                    Some(s) => self.bot_token = Some(s.to_string()),
                    None => {
                        return Err(
                            "Invalid value provided to bot_token in the config file!".into()
                        );
                    }
                },
                "db_path" => {
                    if let Some(db_path) = table["db_path"].as_str() {
                        self.db_path = expanduser(db_path)?
                    }
                }
                "log_path" => match table["log_path"].as_str() {
                    Some(s) => self.log_path = expanduser(s)?,
                    None => {
                        return Err("Invalid value provided to log_path in the config file!".into());
                    }
                },
                "update_time" => {
                    if let Some(update_time) = table["update_time"].as_str() {
                        self.update_time = parse_update_time(update_time).unwrap()
                    }
                }
                _ => (),
            }
        }
        Ok(())
    }

    pub fn apply_args(&mut self, args: &[String]) -> Result<(), Box<dyn Error>> {
        let mut it = args.iter();
        let _ = it.next(); // Skip the first
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "-i" | "--interactive" => self.interactive = true,
                "-d" | "--debug" => self.debugmode = true,
                "-np" | "--non-persistent" => self.persistent = false,
                "-u" | "--update-times" => match it.next() {
                    Some(s) => self.update_time = parse_update_time(s).unwrap(),
                    None => return Err("No timestamps provided after -u!".into()),
                },
                "-f" => match it.next() {
                    Some(f) => self.config_path = expanduser(f)?,
                    None => return Err("No config file name provided after -f!".into()),
                },
                "-p" | "--db-path" => match it.next() {
                    Some(f) => self.db_path = expanduser(f)?,
                    None => return Err("No db path provided after -p / --db-path!".into()),
                },
                "-t" | "--token" => match it.next() {
                    Some(f) => self.bot_token = Some(f.clone()),
                    None => return Err("No bot token name provided after -t / --token!".into()),
                },
                "-l" | "--log-level" => match it.next() {
                    Some(s) => self.log_level = parse_log_level(s)?,
                    None => return Err("No bot token name provided after -l / --log-level!".into()),
                },
                _ => return Err("Unknown argument {arg}".into()),
            }
        }
        Ok(())
    }

    pub fn log_structs(&self) {
        log::info!(
            "bot_token: {}",
            self.bot_token.as_ref().unwrap_or(&"".to_string())
        );
        log::info!("Persistent: {}", self.persistent);
        log::info!("Update times: {:?}", self.update_time);
        log::info!("Interactive mode: {}", self.interactive);
        log::info!("Debug mode: {}", self.debugmode);
        log::info!("Filepath: {:#?}", self.config_path);
        log::info!("Database path: {:#?}", self.db_path);
        log::info!("Log path: {:#?}", self.log_path);
        log::info!("Log level: {:?}", self.log_level);
    }
}

fn parse_log_level(level: &str) -> Result<log::LevelFilter, Box<dyn Error>> {
    match level.to_lowercase().as_str() {
        "off" => Ok(log::LevelFilter::Off),
        "error" => Ok(log::LevelFilter::Error),
        "warn" => Ok(log::LevelFilter::Warn),
        "info" => Ok(log::LevelFilter::Info),
        "debug" => Ok(log::LevelFilter::Debug),
        _ => Err("Invalid log level: off, error, warn, info and debug are valid.".into()),
    }
}

fn parse_update_time(input: &str) -> Result<String, Box<dyn Error>> {
    Ok(String::from(input.trim()))
}

fn _parse_update_time(input: &str) -> Result<Vec<NaiveTime>, Box<dyn Error>> {
    let mut result: Vec<NaiveTime> = Vec::new();
    for i in input.split(",") {
        if i.contains("-") {
            let loc = i.find("-").unwrap();
            let (first_s, last_s) = (&i[..loc], &i[loc + 1..]);
            let (first, last) = (u32::from_str(first_s)?, u32::from_str(last_s)?);
            if first < last {
                for time_num in first..=last {
                    result.push(
                        NaiveTime::from_hms_milli_opt(time_num, 0, 0, 0)
                            .ok_or(format!("Invalid time entered: {time_num}"))?,
                    );
                }
            } else {
                let errormessage = format!(
                    "Error: the first number in a range should be lower than the second\nFirst number:{first}\nSecond number:{last}"
                );
                return Err(errormessage.as_str().into());
            }
        } else {
            let time_num = u32::from_str(i)?;
            result.push(
                NaiveTime::from_hms_milli_opt(time_num, 0, 0, 0)
                    .ok_or(format!("Invalid time entered: {time_num}"))?,
            );
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let result = Config::build(&["".to_string()]).expect("Error!");
        assert!(!result.debugmode);
        assert_eq!(result.config_path.to_str(), Some("config.json"));

        let result = Config::build(&["aaa".to_string(), "-f".to_string()]);
        assert!(result.is_err());

        let argvec: Vec<String> = (["aaa", "-d", "-f", "newconf.json", "-t", "token"])
            .iter()
            .map(|x| x.to_string())
            .collect();

        let result = Config::build(&argvec).expect("Error!");
        assert!(result.debugmode);
        assert_eq!(result.bot_token, Some("token".to_string()));
    }

    #[test]
    fn test_toml_reader() {
        let mut config = Config::default();
        config.apply_toml(Path::new("config.toml")).unwrap();
        assert_eq!(config.admin.unwrap(), 6242952853);
        assert_eq!(config.update_time, parse_update_time("9-17").unwrap());
    }
}
