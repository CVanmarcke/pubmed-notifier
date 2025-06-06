use chrono::ParseError;
use chrono::prelude::*;
use rss::Channel;
use rss::Item;
use rusqlite::types::FromSql;
use rusqlite::types::FromSqlError;
use rusqlite::types::FromSqlResult;
use rusqlite::types::ValueRef;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::str::FromStr;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct ChannelWrapper(Channel);

impl Default for ChannelWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelWrapper {
    pub fn new() -> ChannelWrapper {
        ChannelWrapper(Channel::default())
    }
    pub fn build(channel: Channel) -> ChannelWrapper {
        ChannelWrapper(channel)
    }
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(&self)
    }
    pub fn from_json(json: &str) -> serde_json::Result<ChannelWrapper> {
        serde_json::from_str(json)
    }
    pub fn replace(&mut self, channel: Channel) {
        self.0 = channel;
    }
    pub fn get_new_items<'a>(&'a self, fromdate: &str) -> Result<Vec<&'a Item>, ParseError> {
        let prev: DateTime<FixedOffset> = DateTime::parse_from_rfc2822(fromdate)?;
        let mut new_items: Vec<&Item> = Vec::new();
        for item in self.items() {
            if let Some(pub_date) = item.pub_date() {
                if DateTime::parse_from_rfc2822(pub_date).unwrap() > prev {
                    new_items.push(item);
                }
            }
        }
        Ok(new_items)
    }
    pub fn get_new_items_from_last<'a>(&'a self, guid: &u32) -> Vec<&'a Item> {
        let mut new_items: Vec<&Item> = Vec::new();
        for item in self.items() {
            if ChannelWrapper::parse_guid(item).unwrap_or(0) <= *guid {
                break;
            }
            new_items.push(item);
        }
        new_items
    }

    pub fn parse_guid(item: &Item) -> Result<u32, <u32 as FromStr>::Err> {
        item.guid()
            .as_ref()
            .unwrap()
            .value()
            .trim_start_matches("pubmed:")
            .parse()
    }
}

// impl Serialize for ChannelWrapper {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         serializer.serialize_newtype_struct("ChannelWrapper", &self.0.to_string())
//         // serializer.serialize_str(&self.0.to_string()[..])
//     }
// }

// // https://stackoverflow.com/questions/46753955/how-to-transform-fields-during-deserialization-using-serde
// impl<'de> Deserialize<'de> for ChannelWrapper {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         // let s: &str = Deserialize::deserialize(deserializer)?;
//         let s: String = Deserialize::deserialize(deserializer)?;
//         let channel = Channel::read_from(s.as_bytes()).map_err(D::Error::custom)?;
//         Ok(ChannelWrapper(channel))
//     }
// }

impl FromSql for ChannelWrapper {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Text(s) => serde_json::from_slice(s), // KO for b"text"
            ValueRef::Blob(b) => serde_json::from_slice(b),
            ValueRef::Null => Ok(ChannelWrapper::default()),
            _ => return Err(FromSqlError::InvalidType),
        }
        .map_err(|err| FromSqlError::Other(Box::new(err)))
    }
}

impl Deref for ChannelWrapper {
    type Target = Channel;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn to_json() {
        let cw = ChannelWrapper(Channel::default());
        let json: String = serde_json::to_string(&cw).unwrap();
        println!("{}", json);
        let cw2 = serde_json::from_str(&json).unwrap();
        assert_eq!(cw, cw2)
    }
}
