use crate::datastructs::{ChannelLookupTable, PubmedFeed, User, UserRssList};
use crate::formatter::PreppedMessage;
use crate::preset::{self, available_presets, Keywords};
use crate::{CustomResult, db};
use chrono::NaiveDate;
use rusqlite::Connection;
use teloxide::utils::command::BotCommands;

#[derive(BotCommands, PartialEq, Debug, Clone)]
#[command(
    rename_rule = "lowercase",
    parse_with = "split",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(hide)]
    Start,
    #[command(description = "Display this text.")]
    Help,
    #[command(description = "List the available feeds.", parse_with = "split")]
    Feeds,
    #[command(description = "List how many collections you have.")]
    Collections,
    #[command(
        description = "[collection] - Show the content of a collection (journals, whitelist keywords and blacklist keywords). Provide the collection number, starting at 0 (eg \"/collection 0\")",
        parse_with = "split"
    )]
    Collection { collection_index: usize },
    #[command(description = "Create a new, empty collection", parse_with = "split")]
    NewCollection,
    #[command(
        description = "[name] [link] - Add a new pubmed feed. Provide the name of the feed and link.",
        parse_with = "split"
    )]
    NewFeed { name: String, link: String },
    #[command(
        description = "[feed id] [collection] - Add a feed. Provide the id and collection number. Eg. /addfeed 101532453 0 to add Insights in Imaging to your first feed collection.",
        parse_with = "split"
    )]
    AddFeed {
        feed_id: u32,
        collection_index: usize,
    },
    #[command(
        description = "Add a keyword to the whitelist. Provide the keyword and collection number. Space can be entered by using _. Eg. /addtowhitelist cervical_cancer 0",
        parse_with = "split"
    )]
    AddToWhitelist {
        keyword: String,
        collection_index: usize,
    },
    #[command(
        description = "Add a keyword to the blacklist. Provide the keyword and collection number.",
        parse_with = "split"
    )]
    AddToBlacklist {
        keyword: String,
        collection_index: usize,
    },
    #[command(
        description = "Remove a feed from a collection. Provide the id and collection number.",
        parse_with = "split"
    )]
    RemoveFeed {
        feed_id: u32,
        collection_index: usize,
    },
    #[command(
        description = "Remove a keyword from the whitelist. Provide the keyword and collection number.",
        parse_with = "split"
    )]
    RemoveFromBlacklist {
        keyword: String,
        collection_index: usize,
    },
    #[command(
        description = "Remove a keyword from the blacklist. Provide the keyword and collection number.",
        parse_with = "split"
    )]
    RemoveFromWhitelist {
        keyword: String,
        collection_index: usize,
    },
    #[command(description = "List available presets.", parse_with = "split")]
    Presets,
    #[command(
        description = "Add the content of a preset to a collection. Provide the preset name and collection index.",
        parse_with = "split"
    )]
    AddPresetToCollection {
        preset: String,
        collection_index: usize,
    },
    #[command(hide)]
    GetLastUpdate,
    #[command(hide)]
    SetLastUpdate { date: String }, // in format YYY-mm-dd
    #[command(hide)]
    GetNewSince { date: String }, // in format YYY-mm-dd
}

pub async fn message_handler(
    msg: &str,
    user: &mut User,
    conn: &rusqlite::Connection,
) -> CustomResult<String> {
    let command = Command::parse(msg, "")?; // TODO evt veranderen naar Command::Help
    match command {
          Command::Start => Ok("Welcome to the telegram pubmed notifier bot! Send /help for a list of available commands.".to_string()),
          Command::Help => Ok(Command::descriptions().to_string()),
          Command::Collections => Ok(format!("You currently have {} collections in total. Inspect them with /collection [num]", user.rss_lists.len())) ,
          Command::Collection { collection_index  } => show_collection(conn, user, collection_index),
          Command::Feeds => list_feeds(conn),
          Command::NewFeed { name, link } =>  newfeed(conn, name, link),
          Command::AddFeed { feed_id, collection_index } => add_feed_to_collection(conn, user, feed_id, collection_index),
          Command::AddToWhitelist { keyword, collection_index } => add_to_whitelist(conn, user, keyword, collection_index),
          Command::AddToBlacklist { keyword, collection_index } => add_to_blacklist(conn, user, keyword, collection_index),
          Command::RemoveFeed { feed_id, collection_index } => remove_feed_from_collection(conn, user, feed_id, collection_index),
          Command::RemoveFromWhitelist { keyword, collection_index } => remove_from_whitelist(conn, user, keyword, collection_index),
          Command::RemoveFromBlacklist { keyword, collection_index } => remove_from_blacklist(conn, user, keyword, collection_index),
          Command::NewCollection => new_collection(conn, user),
          Command::Presets => show_presets(),
          Command::AddPresetToCollection { preset, collection_index} => add_preset_to_collection(conn, user, preset, collection_index),
          Command::GetLastUpdate => get_last_update(user),
          Command::SetLastUpdate {date} => set_last_update(conn, user, date),
          Command::GetNewSince {date} => get_new_since(conn, user, date), // in format YYY-mm-dd
      }
}

fn list_feeds(conn: &Connection) -> CustomResult<String> {
    let feeds = db::sqlite::get_feeds(conn)?;
    Ok(ChannelLookupTable::from_vec(feeds)?.format())
}

fn add_feed_to_collection(
    conn: &Connection,
    user: &mut User,
    feed_id: u32,
    collection_index: usize,
) -> CustomResult<String> {
    // Get userCollection, if it exists
    if let Some(coll) = user.rss_lists.get_mut(collection_index) {
        // Check that the feed_id actually corresponds with an existing channel.
        if let Some(feed) = db::sqlite::get_feed(conn, &feed_id)? {
            coll.feeds.insert(feed_id);
            db::sqlite::update_user(conn, user)?;
            return Ok(format!(
                "Added {} ({}) to collection {}.",
                feed_id, feed.name, collection_index
            ));
        }
        return Ok(format!("Feed {} does not exist.", feed_id));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}
fn remove_feed_from_collection(
    conn: &Connection,
    user: &mut User,
    feed_id: u32,
    collection_index: usize,
) -> CustomResult<String> {
    // Get userCollection, if it exists
    if let Some(coll) = user.rss_lists.get_mut(collection_index) {
        if coll.feeds.remove(&feed_id) {
            db::sqlite::update_user(conn, user)?;
            return Ok(format!(
                "Removed {} from collection {}.",
                feed_id, collection_index
            ));
        }
        return Ok(format!(
            "Feed {} was not in collection {}.",
            feed_id, collection_index
        ));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn add_to_whitelist(
    conn: &Connection,
    user: &mut User,
    keyword: String,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(coll) = user.rss_lists.get_mut(collection_index) {
        coll.whitelist.insert(keyword.replace("_", " "));
        db::sqlite::update_user(conn, user)?;
        return Ok(format!(
            "Added '{}' to the whitelist of collection {}.",
            keyword.replace("_", " "),
            collection_index
        ));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn add_to_blacklist(
    conn: &Connection,
    user: &mut User,
    keyword: String,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(coll) = user.rss_lists.get_mut(collection_index) {
        coll.blacklist.insert(keyword.replace("_", " "));
        db::sqlite::update_user(conn, user)?;
        return Ok(format!(
            "Added '{}' to the blacklist of collection {}.",
            keyword.replace("_", " "),
            collection_index
        ));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn remove_from_whitelist(
    conn: &Connection,
    user: &mut User,
    keyword: String,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(coll) = user.rss_lists.get_mut(collection_index) {
        if coll.whitelist.remove(&keyword.replace("_", " ")) {
            db::sqlite::update_user(conn, user)?;
            return Ok(format!(
                "Removed '{}' from the whitelist of collection {}.",
                keyword, collection_index
            ));
        }
        return Ok(format!(
            "'{}' was not in the whitelist of collection {}.",
            keyword.replace("_", " "),
            collection_index
        ));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn remove_from_blacklist(
    conn: &Connection,
    user: &mut User,
    keyword: String,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(coll) = user.rss_lists.get_mut(collection_index) {
        if coll.blacklist.remove(&keyword.replace("_", " ")) {
            db::sqlite::update_user(conn, user)?;
            return Ok(format!(
                "Removed '{}' from the blacklist of collection {}.",
                keyword.replace("_", " "),
                collection_index
            ));
        }
        return Ok(format!(
            "'{}' was not in the blacklist of collection {}.",
            keyword, collection_index
        ));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn show_collection(
    conn: &Connection,
    user: &mut User,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(collection) = user.rss_lists.get(collection_index) {
        db::sqlite::format_collection(conn, collection)
    } else {
        Ok(format!(
            "The index is out of range: pick a number between 0 and {}",
            user.rss_lists.len().saturating_sub(1)
        ))
    }
}

fn newfeed(conn: &Connection, name: String, link: String) -> CustomResult<String> {
    let feed = PubmedFeed::build_from_link(&link, &name)?;
    let uid = db::sqlite::add_feed(conn, &feed)?;
    Ok(format!(
        "Added feed {}, with id {}. Add it to a collection with /addfeed {} [collection_index].",
        name, uid, uid
    ))
}

fn new_collection(conn: &Connection, user: &mut User) -> CustomResult<String> {
    let mut collection = UserRssList::new();
    collection.blacklist = preset::merge_preset_with_set(Keywords::DefaultBlacklist, &collection.blacklist);
    user.rss_lists.push(collection);
    db::sqlite::update_user(conn, user)?;
    Ok(format!(
        "Created new collection with index {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn show_presets() -> CustomResult<String> {
    Ok(format!("Available presets:\n{:?}", available_presets()))
}

fn add_preset_to_collection(
    conn: &Connection,
    user: &mut User,
    preset: String,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(collection) = user.rss_lists.get_mut(collection_index) {
        match preset.as_str() {
            "uro" => {
                collection.whitelist = preset::merge_preset_with_set(Keywords::Uro, &collection.whitelist)
            }
            "abdomen" => {
                collection.whitelist = preset::merge_preset_with_set(Keywords::Abdomen, &collection.whitelist)
            }
            "default_blacklist" => {
                collection.blacklist = preset::merge_preset_with_set(Keywords::DefaultBlacklist, &collection.blacklist)
            }
            "ai_blacklist" => {
                collection.blacklist = preset::merge_preset_with_set(Keywords::AIBlacklist, &collection.blacklist)
            }
            "radiology_journals" => {
                collection.feeds = preset::radiology_journals()
                    .into_iter()
                    .chain(collection.feeds.clone())
                    .collect()
            }
            _ => return Ok(format!("'{}' is not a valid preset!", preset)),
        }
        db::sqlite::update_user(conn, user)?;
        return Ok(format!(
            "Added preset {} to collection {}",
            preset, collection_index
        ));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn get_last_update(user: &User) -> CustomResult<String> {
    Ok(user.last_pushed.clone())
}

fn set_last_update(conn: &Connection, user: &mut User, date: String) -> CustomResult<String> {
    let newdate = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    user.last_pushed = newdate.to_rfc2822();
    db::sqlite::update_user(conn, user)?;
    return Ok(format!("Changed the last updated time to {}", newdate));
}

fn get_new_since(conn: &Connection, user: &User, date: String) -> CustomResult<String> {
    let mut tempuser = user.clone();
    let newdate = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    tempuser.last_pushed = newdate.to_rfc2822();

    let feeds = ChannelLookupTable::from_vec(db::sqlite::get_feeds(conn)?)?;
    if let Some(items) = tempuser.get_new_items(&feeds) {
        for item in items {
            println!("----------------------------");
            println!("{}", PreppedMessage::build(item).format_as_markdownv2());
        }
    }
    Ok(format!("Output to sdt..."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_date() {
        let date = "2025-03-01";
        let newdate = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        assert_eq!(newdate, Utc.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap())
    }
}
