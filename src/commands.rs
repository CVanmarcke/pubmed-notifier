use crate::datastructs::{ChannelLookupTable, ItemMetadata, PubmedFeed, User, UserRssList};
use crate::formatter::PreppedMessage;
use crate::preset::{self, Keywords, Preset, available_presets};
use crate::{CustomResult, db};
use chrono::NaiveDate;
use rusqlite::Connection;
use teloxide::types::ParseMode;
use teloxide::utils::command::{BotCommands, ParseError};

#[derive(BotCommands, PartialEq, Debug, Clone)]
#[command(
    rename_rule = "lowercase",
    parse_with = "split",
    description = "These commands are supported. Note that when you need a space in a keyword or name, you need to type _ instead."
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
        description = "[collection] - Show the journals and keywords of a collection. Provide the collection number, starting at 0 (eg \"/collection 0\")",
        parse_with = "split"
    )]
    Collection { collection_index: usize },
    #[command(description = "Create a new, empty collection", parse_with = "split")]
    NewCollection,
    #[command(
        description = "[collection] - Completely deletes a collection. WARNING: this cannot be undone!)",
        parse_with = "split"
    )]
    DeleteCollection { collection_index: usize },
    #[command(
        description = "[feed_name] [link] - Add a new pubmed feed. Provide the name of the feed (with any spaces replaced by _) and link.",
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
        description = "[word] [collection] - Add a keyword to the whitelist. Provide the keyword and collection number. Space can be entered by using _. Eg. /addtowhitelist cervical_cancer 0",
        parse_with = "split"
    )]
    AddToWhitelist {
        keyword: String,
        collection_index: usize,
    },
    #[command(
        description = "[word] [collection] - Add a keyword to the blacklist. Space can be entered by using _",
        parse_with = "split"
    )]
    AddToBlacklist {
        keyword: String,
        collection_index: usize,
    },
    #[command(
        description = "[id] [collection] - Remove a feed from a collection.",
        parse_with = "split"
    )]
    RemoveFeed {
        feed_id: u32,
        collection_index: usize,
    },
    #[command(
        description = "[word] [collection] - Remove a keyword from the whitelist.",
        parse_with = "split"
    )]
    RemoveFromBlacklist {
        keyword: String,
        collection_index: usize,
    },
    #[command(
        description = "[word] [collection] - Remove a keyword from the blacklist.",
        parse_with = "split"
    )]
    RemoveFromWhitelist {
        keyword: String,
        collection_index: usize,
    },
    #[command(description = "List available presets.", parse_with = "split")]
    Presets,
    #[command(description = "[preset] - Show preset content.", parse_with = "split")]
    Preset { preset: String },
    #[command(
        description = "[preset_name] [collection] - Add the content of a preset to a collection.",
        parse_with = "split"
    )]
    AddPresetToCollection {
        preset: String,
        collection_index: usize,
    },
}

pub async fn user_command_handler(
    msg: &str,
    user: &mut User,
    conn: &rusqlite::Connection,
) -> CustomResult<String> {
    let command = Command::parse(msg, "");
    if command.is_err() {
        return Err(format!(
            "\"{}\" is not a valid command! Send /help to view the list of valid commands.",
            msg
        )
        .into());
    }

    match command.unwrap() {
        Command::Start => Ok("Welcome to the telegram pubmed notifier bot! Send /help for a list of available commands.".to_string()),
        Command::Help => Ok(Command::descriptions().to_string()),
        Command::Collections => Ok(format!("You currently have {} collections in total. Inspect them with /collection [num] (starting at 0).", user.rss_lists.len())) ,
        Command::Collection { collection_index  } => show_collection(conn, user, collection_index),
        Command::Feeds => list_feeds(conn),
        Command::NewFeed { name, link } =>  newfeed(conn, name, link).await,
        Command::AddFeed { feed_id, collection_index } => add_feed_to_collection(conn, user, feed_id, collection_index),
        Command::AddToWhitelist { keyword, collection_index } => add_to_whitelist(conn, user, keyword, collection_index),
        Command::AddToBlacklist { keyword, collection_index } => add_to_blacklist(conn, user, keyword, collection_index),
        Command::RemoveFeed { feed_id, collection_index } => remove_feed_from_collection(conn, user, feed_id, collection_index),
        Command::RemoveFromWhitelist { keyword, collection_index } => remove_from_whitelist(conn, user, keyword, collection_index),
        Command::RemoveFromBlacklist { keyword, collection_index } => remove_from_blacklist(conn, user, keyword, collection_index),
        Command::NewCollection => new_collection(conn, user),
        Command::DeleteCollection { collection_index } => delete_collection(conn, user, collection_index),
        Command::Presets => show_presets(),
        Command::Preset {preset} => show_preset_content(conn, &preset),
        Command::AddPresetToCollection { preset, collection_index} => add_preset_to_collection(conn, user, preset, collection_index),
    }
}

fn get_users(conn: &Connection) -> CustomResult<String> {
    let users = db::sqlite::get_users(conn)?;
    let mut r = "Users:\n".to_string();
    for user in users {
        r.push_str(&format!("{}, ", user.chat_id));
    }
    Ok(r)
}

async fn as_user(conn: &Connection, user_id: i64, msg: &str) -> CustomResult<String> {
    let mut other_user = db::sqlite::get_user(conn, user_id)?;
    if other_user.is_none() {
        return Ok("User does not exist.".to_string());
    }
    // TODO if error, return error, or check if valid command...
    return user_command_handler(msg, other_user.as_mut().unwrap(), conn).await;
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
        if let Some(feed) = db::sqlite::get_feed(conn, feed_id)? {
            coll.feeds.insert(feed_id);
            db::sqlite::update_user(conn, user)?;
            db::sqlite::add_subscriber(conn, feed_id, 1)?;
            return Ok(format!(
                "Added {} ({}) to collection {}.",
                feed_id, feed.name, collection_index
            ));
        }
        return Ok(format!("Feed {} does not exist.", feed_id));
    }
    Ok(format!(
        "The index is out of range: pick a number between 0 and {}, or create a new collection with /newcollection",
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
            db::sqlite::add_subscriber(conn, feed_id, -1)?;
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
        "The index is out of range: pick a number between 0 and {}, or create a new collection with /newcollection",
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
        "The index is out of range: pick a number between 0 and {}, or create a new collection with /newcollection",
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
        "The index is out of range: pick a number between 0 and {}, or create a new collection with /newcollection",
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
        "The index is out of range: pick a number between 0 and {}, or create a new collection with /newcollection",
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
        "The index is out of range: pick a number between 0 and {}, or create a new collection with /newcollection",
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
    } else if user.rss_lists.is_empty() {
        new_collection(conn, user)?;
        Ok("Collection with index 0 did not exist; created a new empty collection with the default blacklist".to_string())
    } else {
        Ok(format!(
            "The index is out of range: pick a number between 0 and {}",
            user.rss_lists.len().saturating_sub(1)
        ))
    }
}

async fn newfeed(conn: &Connection, name: String, link: String) -> CustomResult<String> {
    let mut feed = PubmedFeed::build_from_link(&link, &name)?;
    feed.update_channel_limited().await?;
    feed.update_guid();
    let uid = db::sqlite::add_feed(conn, &feed)?;
    Ok(format!(
        "Added feed {}, with id {}. Add it to a collection with /addfeed {} [collection_index].",
        name, uid, uid
    ))
}

fn new_collection(conn: &Connection, user: &mut User) -> CustomResult<String> {
    let mut collection = UserRssList::new();
    collection.blacklist =
        preset::merge_keyword_preset_with_set(Keywords::DefaultBlacklist, &collection.blacklist);
    user.rss_lists.push(collection);
    db::sqlite::update_user(conn, user)?;
    Ok(format!(
        "Created new collection with index {}",
        user.rss_lists.len().saturating_sub(1)
    ))
}

fn delete_collection(
    conn: &Connection,
    user: &mut User,
    collection_index: usize,
) -> CustomResult<String> {
    if collection_index < user.rss_lists.len() {
        user.rss_lists.remove(collection_index);
        db::sqlite::update_user(conn, user)?;
        Ok(format!(
            "Removed collection with index {}",
            collection_index
        ))
    } else {
        Ok(format!(
            "The collection with index {} does not exist: pick a collection between 0 and {}",
            collection_index,
            user.rss_lists.len().saturating_sub(1)
        ))
    }
}

fn show_presets() -> CustomResult<String> {
    Ok(format!(
        "Available presets:\n{}\n\nAdd them with \"/addpresettocollection [preset_name] [collection_index]\"",
        available_presets()
    ))
}

fn show_preset_content(conn: &Connection, preset_str: &str) -> CustomResult<String> {
    match preset::parse_preset(preset_str) {
        Some(preset) => match preset {
            Preset::Journal(p) => Ok(format!(
                "Content of the preset \"{}\":\n{}",
                preset_str,
                db::sqlite::format_feedlist(conn, &preset::get_preset_journals(p))?
            )),
            Preset::Keyword(p) => Ok(format!(
                "Content of the preset \"{}\":\n{:?}",
                preset_str,
                preset::get_preset_keywords(p)
            )),
        },
        None => Ok(format!("'{}' is not a valid preset!", preset_str)),
    }
}

fn add_preset_to_collection(
    conn: &Connection,
    user: &mut User,
    preset: String,
    collection_index: usize,
) -> CustomResult<String> {
    if let Some(collection) = user.rss_lists.get_mut(collection_index) {
        match preset::parse_preset(&preset) {
            Some(preset) => match preset {
                Preset::Journal(p) => {
                    collection.feeds = preset::merge_journal_preset_with_set(p, &collection.feeds);
                    // TODO: a more elegant solution to update this!
                    db::sqlite::update_subscribers(conn)?;
                }
                Preset::Keyword(p) => match p {
                    Keywords::Uro | Keywords::Abdomen => {
                        collection.whitelist =
                            preset::merge_keyword_preset_with_set(p, &collection.whitelist)
                    }
                    Keywords::DefaultBlacklist | Keywords::AIBlacklist => {
                        collection.blacklist =
                            preset::merge_keyword_preset_with_set(p, &collection.blacklist)
                    }
                },
            },
            None => return Ok(format!("'{}' is not a valid preset!", preset)),
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

fn _set_last_update(conn: &Connection, user: &mut User, date: String) -> CustomResult<String> {
    let newdate = NaiveDate::parse_from_str(&date, "%Y-%m-%d")?
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc();
    user.last_pushed = newdate.to_rfc2822();
    db::sqlite::update_user(conn, user)?;
    Ok(format!("Changed the last updated time to {}", newdate))
}

fn _get_new_since(conn: &Connection, user: &User, date: String) -> CustomResult<String> {
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
            println!(
                "{}",
                PreppedMessage::build(item, &ItemMetadata::default()).format(ParseMode::MarkdownV2)
            );
        }
    }
    Ok("Output to sdt...".to_string())
}

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", parse_with = "split")]
pub enum AdminCommand {
    #[command(description = "Display this text.")]
    AdminHelp,
    #[command(description = "Update all the feeds.")]
    Update,
    #[command(description = "List the users id's in the database.")]
    Users,
    #[command(description = "Execute a command as another user.", parse_with = as_user_parser)]
    AsUser { id: i64, msg: String },
    #[command(description = "[feed_id] [item_index] - Get an item from a feed.")]
    GetItem { feed_id: u32, index: usize },
}

pub async fn admin_command_handler(msg: &str, conn: &rusqlite::Connection) -> CustomResult<String> {
    let command = AdminCommand::parse(msg, "");
    if command.is_err() {
        return Err(format!(
            "\"{}\" is not a valid command! Send /adminhelp to view the list of valid commands.",
            msg
        )
        .into());
    }
    match command.unwrap() {
        AdminCommand::GetItem { feed_id, index } => get_item_from_feed(conn, feed_id, index), // in format YYY-mm-dd
        AdminCommand::AdminHelp => Ok(AdminCommand::descriptions().to_string()),
        AdminCommand::Users => get_users(conn), // in format YYY-mm-dd
        AdminCommand::AsUser { id, msg } => as_user(conn, id, &msg).await, // in format YYY-mm-dd
        AdminCommand::Update => db::sqlite::update_channels(conn)
            .await
            .map(|_| "Updated channels".to_string())
            .map_err(|e| e.into()),
    }
}

fn as_user_parser(s: String) -> Result<(i64, String), ParseError> {
    match s.find(" ") {
        Some(first_space) => {
            let id = s[0..first_space]
                .parse::<i64>()
                .map_err(|e| ParseError::IncorrectFormat(e.into()))?;
            Ok((id, s[first_space + 1..].to_string()))
        }
        None => Err(ParseError::Custom(
            "Wrong command. Provide a UserId and a command, divided with spaces."
                .to_string()
                .into(),
        )),
    }
}

fn get_item_from_feed(conn: &Connection, feed_id: u32, index: usize) -> CustomResult<String> {
    match db::sqlite::get_feed(conn, feed_id)? {
        Some(feed) => {
            match feed.channel.items.get(index) {
                Some(item) => Ok(PreppedMessage::build(item, &ItemMetadata::default())
                    .format(ParseMode::MarkdownV2)),
                None => Ok("Index out of bounds!".to_string()),
            }
        }
        None => Ok("No feed with that id!".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::prelude::*;

    #[test]
    fn test_as_user_parser() {
        assert_eq!(
            as_user_parser("136743 /collection 0".to_string()).unwrap(),
            (136743, "/collection 0".to_string())
        );
        assert!(as_user_parser("aa 1234".to_string()).is_err());
        assert!(as_user_parser("1234".to_string()).is_err());
    }

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
