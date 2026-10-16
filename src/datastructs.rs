use crate::channelwrapper::ChannelWrapper;
use crate::rsshandler::item_contains_keyword;
use chrono::Local;
use chrono::format::ParseResult;
use serde_json::Value;
use core::str;
use futures::future::join_all;
use regex::Regex;
use rss::Channel;
use rss::Item;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::ops::DerefMut;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct User {
    pub chat_id: i64,
    pub full_name: Option<String>,
    pub last_pushed: String, // of date
    pub rss_lists: Vec<UserRssList>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct UserRssList {
    pub feeds: HashSet<u32>,
    pub whitelist: HashSet<String>,
    pub blacklist: HashSet<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PubmedFeed {
    pub name: String,
    pub uid: Option<u32>,
    pub link: String,
    pub channel: ChannelWrapper,
    pub last_pushed_guid: Option<u32>,
    pub subscribers: u32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct ItemMetadata {
    pub collection: Option<usize>,
    pub keyword: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChannelLookupTable(BTreeMap<u32, PubmedFeed>);

impl User {
    pub fn new(chat_id: i64) -> User {
        User {
            chat_id,
            ..Default::default()
        }
    }
    pub fn build(chat_id: i64, full_name: Option<String>, last_pushed: String, rss_lists: Vec<UserRssList>) -> User {
        User {
            chat_id,
            full_name,
            last_pushed,
            rss_lists,
        }
    }
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(&self)
    }
    pub fn build_from_json(json: &str) -> serde_json::Result<User> {
        serde_json::from_str(json)
    }
    // TODO
    pub fn get_new_items<'a>(
        &self,
        feedmap: &'a BTreeMap<u32, PubmedFeed>,
    ) -> Option<Vec<&'a Item>> {
        //TODO
        let mut to_send = Vec::new();
        for list in &self.rss_lists {
            //TODO uid klopt niet
            for uid in &list.feeds {
                if let Some(pmfeed) = feedmap.get(uid) {
                    if let Ok(items) = pmfeed.channel.get_new_items(&self.last_pushed) {
                        log::trace!(
                            "Collected {} new items in feed {} for user {}",
                            items.len(),
                            uid,
                            self.chat_id
                        );
                        // for item in items
                        items
                            .into_iter()
                            .inspect(|item| log::debug!("Title: {}", item.title().unwrap_or("")))
                            .filter(|item| item_contains_keyword(item, &list.whitelist))
                            // .inspect(|item| log::debug!("item passed whitelist: {}", item.title().unwrap()))
                            .filter(|item| !item_contains_keyword(item, &list.blacklist))
                            // .inspect(|item| log::debug!("Item is included to send: {}", item.title().unwrap()))
                            .for_each(|item| to_send.push(item))
                        // {
                        //        to_send.push(item)
                        //                        }
                    }
                }
            }
        }
        Some(to_send)
        // None
        // TODO mut self -> edit last updated
    }

    pub fn add_feed(
        &mut self,
        collection_index: usize,
        uid: u32,
    ) -> Result<(), Box<dyn Error + 'static>> {
        // DOES NOT CHECK IF THE UID IS VALID!!
        if let Some(collection) = self.rss_lists.get_mut(collection_index) {
            collection.feeds.insert(uid);
            Ok(())
        } else {
            Err("Index out of bounds.".into())
        }
    }

    pub fn update_last_pushed(&mut self) {
        self.last_pushed = Local::now().to_rfc2822();
    }
}

impl Default for User {
    fn default() -> Self {
        User {
            chat_id: 0,
            full_name: None,
            last_pushed: Local::now().to_rfc2822(),
            rss_lists: Vec::new(),
        }
    }
}

impl PubmedFeed {
    // let link = "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532453/?limit=5&name=Insights%20Imaging&utm_campaign=journals";
    pub async fn download_channel(&self, api_key: Option<&str>) -> Result<ChannelWrapper, Box<dyn Error + Sync + Send>> {
let client = reqwest::Client::new();
        
        // Stap 1: Zoek de laatste PMIDs op basis van de journalnaam (term)
        let search_url = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi";
        let mut search_params = vec![
            ("db", "pubmed".to_string()),
            ("term", format!("\"{}\"[ta]", self.name)),
            ("sort", "pub_date".to_string()),
            ("retmax", "20".to_string()),
            ("retmode", "json".to_string()),
            ("tool", "rust_pubmed_scraper".to_string()),
        ];
        if let Some(key) = api_key {
            search_params.push(("api_key", key.to_string()));
        }

        let search_resp = client
            .get(search_url)
            .query(&search_params)
            .send()
            .await?
            .json::<Value>()
            .await?;

        let id_list = search_resp["esearchresult"]["idlist"]
            .as_array()
            .ok_or("No articles found or error in search response.")?;

        let pmids: Vec<String> = id_list
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if pmids.is_empty() {
            return Ok(ChannelWrapper::new());
        }

        // Stap 2: Haal de samenvattingen op voor deze PMIDs
        let summary_url = "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi";
        let pmid_str = pmids.join(",");
        let mut summary_params = vec![
            ("db", "pubmed".to_string()),
            ("id", pmid_str),
            ("retmode", "json".to_string()),
            ("tool", "rust_pubmed_scraper".to_string()),
        ];
        if let Some(key) = api_key {
            summary_params.push(("api_key", key.to_string()));
        }

        let summary_resp = client
            .get(summary_url)
            .query(&summary_params)
            .send()
            .await?
            .json::<Value>()
            .await?;

        let result_obj = summary_resp["result"].as_object().ok_or("Invalid summary response format")?;

        // Converteer de API-resultaten naar RSS Items binnen de ChannelWrapper
        let mut items = Vec::new();
        for pmid in &pmids {
            if let Some(article) = result_obj.get(pmid) {
                let pubdate = article["pubdate"].as_str().unwrap_or("");
                let title = article["title"].as_str().unwrap_or("No Title")
                    .replace("<i>", "").replace("</i>", "")
                    .replace("<b>", "").replace("</b>", "");

                let mut item = Item::default();
                item.set_title(title);
                item.set_guid(rss::Guid {
                    value: pmid.clone(),
                    permalink: false,
                });
                item.set_pub_date(pubdate.to_string());
                item.set_link(format!("https://pubmed.ncbi.nlm.nih.gov/{}/", pmid));
                items.push(item);
            }
        }

        let mut channel = Channel::default();
        channel.set_items(items);
        Ok(ChannelWrapper::build(channel))
    }

    // TODO is double function
    pub async fn update_channel(&mut self, api_key: Option<&str>) -> Result<&PubmedFeed, Box<dyn Error + Sync + Send>> {
        log::info!("Updating feed {} ({:?}) via API...", &self.name, &self.uid);
        let newchannel = self.download_channel(api_key).await?;
        self.channel = newchannel;
        log::info!("... Successfully updated");
        Ok(self)
    }

    pub async fn update_channel_limited(&mut self, api_key: Option<&str>) -> Result<(), Box<dyn Error + Sync + Send>> {
        if self.subscribers == 0 {
            log::debug!("Subscribers is 0, skipping update.");
            return Ok(());
        }
        let newchannel = self.download_channel(api_key).await?;
        self.channel = newchannel;
        log::debug!("Successfully updated channel {}", &self.name);
        Ok(())
    }

    pub fn set_uid(&mut self, newuid: u32) -> Option<u32> {
        self.uid.replace(newuid) // Returns the old value!
    }

    pub fn get_new_items<'a>(&'a self, fromdate: &str) -> ParseResult<Vec<&'a Item>> {
        self.channel.get_new_items(fromdate)
    }

    pub fn get_new_items_from_last(&self) -> Vec<&Item> {
        if let Some(guid) = self.last_pushed_guid.as_ref() {
            self.channel.get_new_items_from_last(guid)
        } else {
            // self.update_guid();
            vec![]
        }
    }

    pub fn update_guid(&mut self) -> Option<u32> {
        let firstitem = self.channel.items().iter().next();
        if let Some(item) = firstitem {
            log::debug!(
                "Found guid of newest item {:?} in {}",
                &item.guid,
                &self.name
            );
            if item.guid().is_some() {
                let guid = ChannelWrapper::parse_guid(item).ok();
                self.last_pushed_guid = guid;
                return guid;
            }
        }
        log::debug!("Could not find a guid in feed {}", &self.name);
        None
    }

    pub fn get_link(&self) -> &String {
        &self.link
    }
    // CAVE: a wrong feed can be inserted
    pub fn build_from_link(link: &str, name: &str) -> Result<PubmedFeed, &'static str> {
        let link = link.to_string();
        if !link.contains("://pubmed.ncbi.nlm.nih.gov/rss/") {
            return Err("Link provided is not a valid pubmed RSS feed!");
        }
        let re =
            Regex::new(r"://pubmed.ncbi.nlm.nih.gov/rss/journals/([0-9]+)/.*?limit=([0-9]+).*$")
                .unwrap();
        let uid;
        if let Some(caps) = re.captures(&link) {
            uid = Some(caps[1].parse::<u32>().unwrap())
        } else {
            uid = None
        };
        Ok(PubmedFeed {
            name: name.to_string(),
            uid,
            link,
            channel: ChannelWrapper::new(),
            last_pushed_guid: None,
            subscribers: 0u32,
        })
    }
    pub fn key(&self) -> &String {
        self.get_link()
    }
}

impl Hash for PubmedFeed {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.key().hash(state);
    }
}

impl PartialEq for PubmedFeed {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl UserRssList {
    pub fn new() -> UserRssList {
        UserRssList {
            feeds: HashSet::new(),
            whitelist: HashSet::new(),
            blacklist: HashSet::new(),
        }
    }

    pub fn filter_item(&self, item: &Item) -> bool {
        item_contains_keyword(item, &self.whitelist)
            && !item_contains_keyword(item, &self.blacklist)
    }

    pub fn filter_items<'a>(&self, items: Vec<&'a Item>) -> Vec<&'a Item> {
        items
            .into_iter()
            .inspect(|item| log::debug!("Title: {}", item.title().unwrap()))
            .filter(|item| item_contains_keyword(item, &self.whitelist))
            // .inspect(|item| log::debug!("item passed whitelist: {}", item.title().unwrap()))
            .filter(|item| !item_contains_keyword(item, &self.blacklist))
            // .inspect(|item| log::debug!("Item is included to send: {}", item.title().unwrap()))
            .collect::<Vec<&'a Item>>()
        // .for_each(|item| to_send.push(item))
        // {
        //        to_send.push(item)
        //                        }

        // todo!();
    }
}

impl Default for UserRssList {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for ChannelLookupTable {
    type Target = BTreeMap<u32, PubmedFeed>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for ChannelLookupTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl From<BTreeMap<u32, PubmedFeed>> for ChannelLookupTable {
    fn from(btreemap: BTreeMap<u32, PubmedFeed>) -> Self {
        ChannelLookupTable(btreemap)
    }
}

impl Default for ChannelLookupTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelLookupTable {
    pub fn new() -> ChannelLookupTable {
        ChannelLookupTable(BTreeMap::new())
    }
    pub fn from_vec(
        vec: Vec<PubmedFeed>,
    ) -> Result<ChannelLookupTable, Box<dyn Error + Send + Sync>> {
        // Will panic if one of the pubmedfeeds does not have a uid!
        let tree = vec.into_iter().map(|item| {
            if item.uid.is_some() {
                Ok((item.uid.unwrap(), item))
            } else {
                Err("Some items do not have a uid! Add them manually with ChannelLookupTable::add.")
            }
        }).collect::<Result<BTreeMap<u32, PubmedFeed>, &str>>();
        Ok(ChannelLookupTable(tree?))
    }

    pub fn format(&self) -> String {
        let mut s = String::from("[");
        for (key, pmf) in self.iter() {
            s.push_str(format!("{}:\n  id: {}\n  link: {}\n", pmf.name, key, pmf.link).as_str())
        }
        s.push(']');
        s
    }
    // TODO from implementeren (van btreemap)
    pub fn add(&mut self, feed: PubmedFeed) -> u32 {
        match feed.uid {
            Some(uid) => {
                self.insert(uid, feed);
                uid
            }
            None => {
                // TODO
                let uid = self.get_unused_key();
                self.insert(uid, feed);
                uid
            }
        }
    }
    fn get_unused_key(&self) -> u32 {
        //TODO
        42
    }
    // TODO result nog
    pub async fn update_all(&mut self, api_key: Option<&str>) -> Vec<Result<&PubmedFeed, Box<dyn Error + Sync + Send>>> {
        let mut futureresults = Vec::new();
        for (_, feed) in self.iter_mut() {
            // Error nog mappen met key er in zodat we weten wat er niet gelukt was....
            futureresults.push(PubmedFeed::update_channel(feed, api_key));
        }
        join_all(futureresults).await
    }
}

#[cfg(test)]
mod tests {
    use crate::preset::{self, Keywords};

    use super::*;

    #[test]
    fn test_jsonconvert() {
        let mut uro_rss_list: UserRssList = UserRssList::new();
        uro_rss_list.whitelist =
            preset::merge_keyword_preset_with_set(Keywords::Uro, &uro_rss_list.whitelist);

        let user = User {
            chat_id: 1234i64,
            full_name: None,
            last_pushed: "31 sept 2024".to_string(),
            rss_lists: vec![uro_rss_list],
        };
        println!("{:?}", &user);
        let cloned_json = user.to_json().unwrap();
        assert_eq!(User::build_from_json(&cloned_json).unwrap(), user);
    }

    #[test]
    fn feed_contains_test() {
        let journal1 = PubmedFeed {
	    name: "Insights Imaging".to_string(),
            link: "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532453/?limit=5&name=Insights%20Imaging&utm_campaign=journals".to_string(),
	    uid: Some(101532453),
	    channel: ChannelWrapper::new(),
            last_pushed_guid: None,
            subscribers: 0};
        let journal2 = PubmedFeed {
	    name: "something else".to_string(),
            link: "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532454/?limit=5&utm_campaign=journals".to_string(),
	    uid: Some(100000),
	    channel: ChannelWrapper::new(),
            last_pushed_guid: None,
            subscribers: 0};
        let journal11 = PubmedFeed {
	    name: "something else".to_string(),
            link: "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532453/?limit=5&name=Insights%20Imaging&utm_campaign=journals".to_string(),
	    uid: Some(101532453),
	    channel: ChannelWrapper::new(),
            last_pushed_guid: None,
            subscribers: 0};
        assert_eq!(journal1, journal11);
        let vec = vec![journal1, journal2];
        assert!(vec.contains(&journal11));
    }
}
