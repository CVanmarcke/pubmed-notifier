use std::{env, error::Error, path::PathBuf};
use std::str::FromStr;

use chrono::NaiveTime;

#[derive(Debug)]
pub struct Config {
    pub debugmode: bool,
    pub interactive: bool,
    pub filepath: PathBuf,
    pub db_path: PathBuf,
    pub bot_token: Option<String>,
    pub persistent: bool,
    pub update_time: Vec<NaiveTime>,
    pub log_level: log::LevelFilter
}
impl Default for Config {
    fn default() -> Self {
        Config {
            debugmode: false,
            interactive: false,
            // TODO support for config.toml
            filepath: PathBuf::from("config.toml"),
            db_path: PathBuf::from("database.db3"),
            bot_token: None,
            persistent: true,
            update_time: parse_update_time("9-17").unwrap(),
            log_level: log::LevelFilter::Info
        }
    }
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config, Box<dyn Error>> {
        let bot_token = env::var("TELOXIDE_TOKEN").ok();
        let mut argstruct: Config = Config { bot_token, ..Default::default() };
        argstruct.apply_args(args)?;

        Ok(argstruct)
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
                    Some(s) => self.update_time = parse_update_time(s).unwrap()
,
                    None => return Err("No timestamps provided after -u!".into())},
                "-f" => match it.next() {
                    Some(f) => self.filepath = PathBuf::from(f),
                    None => return Err("No config file name provided after -f!".into())},
                "-p" | "--db-path" => match it.next() {
                    Some(f) => self.db_path = PathBuf::from(f),
                    None => return Err("No db path provided after -p / --db-path!".into())},
                "-t" | "--token" => match it.next() {
                    Some(f) => self.bot_token = Some(f.clone()),
                    None
 => return Err("No bot token name provided after -t / --token!".into())},
                "-l" | "--log-level" => match it.next() {
                    Some(s) => self.log_level = parse_log_level(s)?,
                    None => return Err("No bot token name provided after -l / --log-level!".into())},
                _ => return Err("Unknown argument {arg}".into())
            }}
        Ok(())
    }

    pub fn log_structs(&self) -> () {
        log::info!("Debug mode: {}", self.debugmode);
        log::info!("Interactive mode: {}", self.interactive);
        log::info!("Filepath: {:#?}", self.filepath);
        log::info!("Database path: {:#?}", self.db_path);
        log::info!("bot_token: {}", self.bot_token.as_ref().unwrap_or(&"".to_string()));
        log::info!("Persistent: {}", self.persistent);
        log::info!("Update times: {:?}", self.update_time);
        log::info!("Log level: {:?}", self.log_level);
    }
}



fn parse_log_level(level: &str) -> Result<log::LevelFilter, Box<dyn Error>> {
    match level.to_lowercase().as_str() {
        "off" => Ok(log::LevelFilter::Off),
        "error" => Ok(log::LevelFilter::Error),
        "warn" =>  Ok(log::LevelFilter::Warn),
        "info" =>  Ok(log::LevelFilter::Info),
        "debug" => Ok(log::LevelFilter::Debug),
        _ => Err("Invalid log level: off, error, warn, info and debug are valid.".into())
    }
}

fn parse_update_time(input: &str) -> Result<Vec<NaiveTime>, Box<dyn Error>> {
    let mut result:  Vec<NaiveTime> = Vec::new();
    for i in input.split(",") {
        if i.contains("-") {
            let loc = i.find("-").unwrap();
            let (first_s, last_s) = (&i[..loc], &i[loc +1 ..]);
            let (first, last) = (u32::from_str(first_s)?, u32::from_str(last_s)?);
            if first < last {
                for time_num in first..=last {
                    result.push(NaiveTime::from_hms_milli_opt(time_num, 0, 0, 0)
                        .ok_or(format!("Invalid time entered: {time_num}"))?);
                }
            } else {
                let errormessage = format!("Error: the first number in a range should be lower than the second\nFirst number:{first}\nSecond number:{last}");
                return Err(errormessage.as_str().into());
            }
        } else {
            let time_num = u32::from_str(i)?;
                    result.push(NaiveTime::from_hms_milli_opt(time_num, 0, 0, 0)
                        .ok_or(format!("Invalid time entered: {time_num}"))?);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_constructor() {
        let t = parse_update_time("5,8-11,16-18").unwrap(); 
        assert_eq!(t,
            vec![NaiveTime::from_hms_milli_opt(5, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(8, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(9, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(10, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(11, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(16, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(17, 0, 0, 0).unwrap(),
                NaiveTime::from_hms_milli_opt(18, 0, 0, 0).unwrap(),
            ]);
        assert!(parse_update_time("9-17").is_ok());
        assert!(parse_update_time("25").is_err());
        assert!(parse_update_time("19-17").is_err());

    }
    #[test]
    fn test_config() {
        let result = Config::build(&vec!["".to_string()]).expect("Error!");
        assert_eq!(result.debugmode, false);
        assert_eq!(result.filepath.to_str(), Some("config.json"));

        let result = Config::build(&vec!["aaa".to_string(), "-f".to_string()]);
        assert!(result.is_err());

        let argvec: Vec<String> = (vec!["aaa", "-d", "-f", "newconf.json", "-t", "token"]).iter().map(|x| x.to_string()).collect();

        let result = Config::build(&argvec).expect("Error!");
        assert_eq!(result.debugmode, true);
        assert_eq!(result.bot_token, Some("token".to_string()));
    }
}
