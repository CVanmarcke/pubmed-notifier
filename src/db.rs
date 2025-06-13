const DB_VERSION: u32 = 1;

pub mod sqlite {
    use crate::db::DB_VERSION;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use rusqlite::{Connection, DatabaseName, Result, params};
    use teloxide::RequestError;
    use tokio_rusqlite;
    // use tokio_rusqlite;
    use crate::channelwrapper::ChannelWrapper;
    use crate::datastructs::User;
    use crate::datastructs::{PubmedFeed, UserRssList};
    use crate::make_feedlist;

    pub fn open(path: &str) -> Result<Connection> {
        let conn = Connection::open(path)?;
        update_db(&conn)?;
        Ok(conn)
    }

    pub fn new(path: &str) -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open(path)?;
        populate(&conn)?;
        Ok(conn)
    }

    pub fn new_in_mem() -> Result<Connection, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        populate(&conn)?;
        Ok(conn)
    }

    pub fn populate(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS users (
            id           INTEGER PRIMARY KEY,
            last_pushed  TEXT NOT NULL,
            collections  TEXT NOT NULL
        )",
            (), // empty list of parameters.
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS feeds (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            name          TEXT NOT NULL,
            link          TEXT NOT NULL UNIQUE,
            channel       TEXT NOT NULL,
            last_pushed_guid   INTEGER,
            subscribers   INTEGER
        )",
            (), // empty list of parameters.
        )?;
        conn.pragma_update(Some(DatabaseName::Main), "user_version", DB_VERSION)?;

        for feed in make_feedlist() {
            add_feed(conn, &feed)?;
        }
        Ok(())
    }

    pub async fn tokio_rusqlite_call<'a, T, F>(
        conn: &'a tokio_rusqlite::Connection,
        func: F,
    ) -> Result<T, RequestError>
    where
        F: FnOnce(&Connection) -> Result<T, rusqlite::Error> + std::marker::Send + 'a + 'static,
        T: std::marker::Send + 'a + 'static,
    {
        conn.call(|conn| {
            let result = func(conn);
            result.map_err(|e| tokio_rusqlite::Error::Other(e.into()))
        })
        .await
        .map_err(|e| RequestError::Io(Arc::new(std::io::Error::other(e))))
    }

    pub fn update_db(conn: &Connection) -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT user_version FROM pragma_user_version;")?;
        let mut rows = stmt.query([])?;
        let row_opt = rows.next()?;
        let version: u32 = match row_opt {
            Some(row) => row.get(0).unwrap_or(0u32),
            None => 0u32,
        };
        if version == DB_VERSION {
            return Ok(());
        }
        log::info!(
            "DB is out of date ({}, should be {}). Updating.",
            version,
            DB_VERSION
        );
        conn.backup(
            DatabaseName::Main,
            format!("{}.bak", &conn.path().unwrap()),
            None,
        )?;
        log::info!("Backup complete");
        if version == 0 {
            log::info!("Adding subscribers column...");
            conn.execute(
                "ALTER TABLE feeds
                   ADD subscribers   INTEGER;",
                (), // empty list of parameters.
            )?;
            log::info!("Updating the subscriber column...");
            update_subscribers(conn)?;

            log::info!("Updating the channel column...");
            let mut stmt =
                conn.prepare("SELECT id, name, link, last_pushed_guid, subscribers FROM feeds")?;
            let feed_iter = stmt.query_map([], |row| {
                Ok(PubmedFeed {
                    name: row.get(1)?,
                    uid: Some(row.get(0)?),
                    link: row.get(2)?,
                    channel: ChannelWrapper::new(),
                    last_pushed_guid: row.get(3)?,
                    subscribers: row.get(4).unwrap_or(0),
                })
            })?;
            for feed in feed_iter {
                update_feed(conn, &feed?)?;
            }
            log::info!("Update to db version 1 complete.");
        }

        log::info!("Done. Updating db_version");
        conn.pragma_update(Some(DatabaseName::Main), "user_version", DB_VERSION)
    }

    pub fn add_subscriber(
        conn: &Connection,
        feed_uid: u32,
        by: i32,
    ) -> Result<usize, rusqlite::Error> {
        let statement = format!(
            "UPDATE feeds SET subscribers = subscribers {} (?1) WHERE id=(?2)",
            match by.is_positive() {
                true => "+",
                false => "-",
            }
        );
        let mut stmt = conn.prepare_cached(&statement)?;
        stmt.execute(params![by.abs(), feed_uid])
    }

    pub fn set_subscribers(
        conn: &Connection,
        feed_uid: u32,
        subscribers: u32,
    ) -> Result<usize, rusqlite::Error> {
        let mut stmt = conn.prepare_cached(
            "UPDATE feeds
                 SET subscribers = ?1
                 WHERE id = ?2",
        )?;
        stmt.execute(params![subscribers, feed_uid])
    }

    pub fn update_subscribers(conn: &Connection) -> Result<(), rusqlite::Error> {
        let users = get_users(conn)?;
        let mut map: HashMap<u32, u32> = HashMap::new();
        for user in users {
            for collection in user.rss_lists {
                for feed_uid in collection.feeds {
                    map.entry(feed_uid).and_modify(|n| *n += 1).or_insert(1);
                }
            }
        }
        for (key, value) in map.into_iter() {
            set_subscribers(conn, key, value)?;
        }
        Ok(())
    }

    pub fn add_user(conn: &Connection, user: &User) -> Result<usize, rusqlite::Error> {
        let collections = serde_json::to_string(&user.rss_lists)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
        conn.execute(
            "INSERT OR IGNORE INTO users (id, last_pushed, collections) VALUES (?1, ?2, ?3)",
            (&user.chat_id, &user.last_pushed, &collections),
        )
    }

    pub fn update_user(conn: &Connection, user: &User) -> Result<usize, rusqlite::Error> {
        let collections = serde_json::to_string(&user.rss_lists)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
        log::debug!("Updating user {} in the database", user.chat_id);
        conn.execute(
            "UPDATE users
             SET last_pushed = ?1,
                 collections = ?2
             WHERE id = ?3",
            params![&user.last_pushed, &collections, &user.chat_id],
        )
    }

    pub fn get_user(conn: &Connection, id: i64) -> Result<Option<User>, rusqlite::Error> {
        let mut stmt =
            conn.prepare("SELECT id, last_pushed, collections FROM users WHERE id=(?1)")?;
        let mut rows = stmt.query([id])?;
        let row_opt = rows.next()?;
        if let Some(row) = row_opt {
            Ok(Some(User {
                chat_id: row.get(0)?,
                last_pushed: row.get(1)?,
                rss_lists: {
                    let s: String = row.get(2)?;
                    serde_json::from_str(s.as_str())
                        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?
                },
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_users(conn: &Connection) -> Result<Vec<User>, rusqlite::Error> {
        let mut stmt = conn.prepare("SELECT id, last_pushed, collections FROM users")?;
        let user_iter = stmt.query_map([], |row| {
            Ok(User {
                chat_id: row.get(0)?,
                last_pushed: row.get(1)?,
                rss_lists: {
                    let s: String = row.get(2)?;
                    serde_json::from_str(s.as_str())
                        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?
                },
            })
        })?;
        user_iter
            .into_iter()
            .collect::<Result<Vec<User>, rusqlite::Error>>()
    }

    pub fn add_feed(conn: &Connection, feed: &PubmedFeed) -> Result<u32, rusqlite::Error> {
        let channel = serde_json::to_string(&feed.channel)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
        if feed.uid.is_some() {
            conn.execute(
                "INSERT OR IGNORE INTO feeds (id, name, link, channel, last_pushed_guid, subscribers) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (&feed.uid.unwrap(), &feed.name, &feed.link, &channel, &feed.last_pushed_guid, &feed.subscribers),
            )?;
            Ok(feed.uid.unwrap())
        } else {
            log::info!(
                "Adding new non-journal feed {} with link {}",
                &feed.name,
                &feed.link
            );
            conn.execute(
                "INSERT OR IGNORE INTO feeds (name, link, channel, last_pushed_guid, subscribers) VALUES (?1, ?2, ?3, ?4, ?5)",
                (&feed.name, &feed.link, &channel, &feed.last_pushed_guid, &feed.subscribers),
            )?;

            let mut stmt = conn.prepare("SELECT id FROM feeds WHERE link=(?1)")?;
            let mut rows = stmt.query([&feed.link])?;
            let row = rows.next()?.ok_or(rusqlite::Error::InvalidParameterName(
                "Couldnt find user".to_string(),
            ))?;
            Ok(row.get(0)?)
        }
    }

    pub fn update_guid_feeds(
        conn: &Connection,
        feeds: &[PubmedFeed],
    ) -> Result<(), rusqlite::Error> {
        // Applies everything, but does not interrupt when there is an error
        let result: Result<Vec<()>, rusqlite::Error> = feeds
            .iter()
            .map(|feed| update_guid_feed(conn, feed))
            .collect();
        result.map(|_| ())
    }

    pub fn update_guid_feed(conn: &Connection, feed: &PubmedFeed) -> Result<(), rusqlite::Error> {
        let mut stmt = conn.prepare_cached(
            "UPDATE feeds
                 SET last_pushed_guid = ?1
                 WHERE id = ?2",
        )?;
        stmt.execute(params![&feed.last_pushed_guid, &feed.uid.unwrap(),])?;
        log::debug!("Updated last_pushed_guid of feed {}", &feed.name);
        Ok(())
    }

    pub fn update_feed(conn: &Connection, feed: &PubmedFeed) -> Result<u32, rusqlite::Error> {
        let channel = serde_json::to_string(&feed.channel)
            .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?;
        if feed.uid.is_some() {
            let mut stmt = conn.prepare_cached(
                "UPDATE feeds
                 SET id = ?1,
                     name = ?2,
                     link = ?3,
                     channel = ?4,
                     last_pushed_guid = ?5,
                     subscribers = ?6
                 WHERE id = ?1",
            )?;
            stmt.execute(params![
                &feed.uid.unwrap(),
                &feed.name,
                &feed.link,
                &channel,
                &feed.last_pushed_guid,
                &feed.subscribers,
            ])?;
            Ok(feed.uid.unwrap())
        } else {
            Err(rusqlite::Error::InvalidParameterName(
                "The provided feed does not have a uid!".to_string(),
            ))
        }
    }

    pub async fn update_channels(conn: &Connection) -> Result<u32, rusqlite::Error> {
        log::info!("Updating all channels...");
        let mut feeds = get_feeds(conn)?;
        let mut result = Vec::new();
        let mut acc = 0;
        for feed in feeds.iter_mut() {
            result.push(feed.update_channel_limited().await);
            let r = update_feed(conn, feed);
            match r {
                Ok(i) => acc += i,
                // TODO
                _ => log::error!("Error updating the database with the updated feed:{:?}", r),
            }
        }
        for r in result {
            if let Err(e) = r {
                log::error!("Error in update_channel_limited function:\n{:?}", e)
            }
        }
        Ok(acc)
    }

    pub fn get_feed(conn: &Connection, id: u32) -> Result<Option<PubmedFeed>, rusqlite::Error> {
        let mut stmt = conn
            .prepare("SELECT id, name, link, channel, last_pushed_guid, subscribers FROM feeds WHERE id=(?1)")?;
        let mut rows = stmt.query([id])?;
        let row_opt = rows.next()?;
        if let Some(row) = row_opt {
            Ok(Some(PubmedFeed {
                name: row.get(1)?,
                uid: Some(row.get(0)?),
                link: row.get(2)?,
                channel: row.get(3)?,
                last_pushed_guid: row.get(4)?,
                subscribers: row.get(5).unwrap_or(0),
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_feeds(conn: &Connection) -> Result<Vec<PubmedFeed>, rusqlite::Error> {
        let mut stmt = conn
            .prepare("SELECT id, name, link, channel, last_pushed_guid, subscribers FROM feeds")?;
        let feed_iter = stmt.query_map([], |row| {
            Ok(PubmedFeed {
                name: row.get(1)?,
                uid: Some(row.get(0)?),
                link: row.get(2)?,
                channel: {
                    let s: String = row.get(3)?;
                    ChannelWrapper::from_json(&s)
                        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?
                },
                last_pushed_guid: row.get(4)?,
                subscribers: row.get(5).unwrap_or(0),
            })
        })?;

        let res: Result<Vec<PubmedFeed>, rusqlite::Error> = feed_iter.into_iter().collect();
        res
    }

    // TODO: verplaatsen naar format module
    // TODO: functie die in één keer een reeks feeds kan ophalen (in een vec)
    pub fn format_feedlist(
        conn: &Connection,
        uids: &HashSet<u32>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut result = String::from("");
        for uid in uids.iter() {
            result.push_str(
                format!(
                    "{}: {}, ",
                    &uid,
                    match get_feed(conn, *uid)? {
                        Some(feed) => feed.name,
                        None => "Corresponding feed not found.".to_string(),
                    }
                )
                .as_str(),
            );
        }
        Ok(result)
    }

    pub fn format_collection(
        conn: &Connection,
        collection: &UserRssList,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut s = String::new();
        let feedstring = format_feedlist(conn, &collection.feeds)?;
        s.push_str(&format!("Feeds: {{ {} }}\n", feedstring));
        s.push_str(&format!("Whitelist: {:?}\n", collection.whitelist));
        s.push_str(&format!("Blacklist: {:?}\n", collection.blacklist));
        Ok(s)
    }
}

#[cfg(test)]
mod tests {

    use simple_expand_tilde::expand_tilde;
    use teloxide::types::ParseMode;

    use crate::{datastructs::ItemMetadata, formatter::PreppedMessage};

    use super::*;

    #[test]
    fn test_db_update() {
        let conn = sqlite::open("target/debug/database.db3").unwrap();
        sqlite::update_db(&conn).unwrap();
    }
    #[test]
    fn test_read_channel() {
        let target = expand_tilde("target/debug/database.db3").unwrap();
        let conn = sqlite::open(target.to_str().unwrap()).unwrap();
        let feed = sqlite::get_feed(&conn, 401260).unwrap().unwrap();
        let item = feed.channel.items.first().unwrap();
        let message =
            PreppedMessage::build(item, &ItemMetadata::default()).format(ParseMode::MarkdownV2);
        println!("{}", message);
    }
}
