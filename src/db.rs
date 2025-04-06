pub mod sqlite {
    use std::collections::HashSet;

    use rusqlite::{Connection, Result, params};
    use teloxide::RequestError;
    use tokio_rusqlite;
    // use tokio_rusqlite;
    use crate::channelwrapper::ChannelWrapper;
    use crate::datastructs::User;
    use crate::datastructs::{PubmedFeed, UserRssList};
    use crate::make_feedlist;

    pub fn open(path: &str) -> Result<Connection> {
        Connection::open(path)
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
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            link        TEXT NOT NULL UNIQUE,
            channel     TEXT NOT NULL,
            last_pushed_guid   INTEGER
        )",
            (), // empty list of parameters.
        )?;
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
            Ok(result.map_err(|e| tokio_rusqlite::Error::Other(e.into()))?)
        })
        .await
        .map_err(|e| RequestError::Io(std::io::Error::other(e.to_string())))
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
                "INSERT OR IGNORE INTO feeds (id, name, link, channel, last_pushed_guid) VALUES (?1, ?2, ?3, ?4, ?5)",
                (&feed.uid.unwrap(), &feed.name, &feed.link, &channel, &feed.last_pushed_guid),
            )?;
            Ok(feed.uid.unwrap())
        } else {
            log::info!(
                "Adding new non-journal feed {} with link {}",
                &feed.name,
                &feed.link
            );
            conn.execute(
                "INSERT OR IGNORE INTO feeds (name, link, channel, last_pushed_guid) VALUES (?1, ?2, ?3, ?4)",
                (&feed.name, &feed.link, &channel, &feed.last_pushed_guid),
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
        feeds: &Vec<PubmedFeed>,
    ) -> Result<(), rusqlite::Error> {
        // Applies everything, but does not interrupt when there is an error
        let result: Result<Vec<()>, rusqlite::Error> = feeds
            .iter()
            .map(|feed| update_guid_feed(conn, feed))
            .collect();
        return result.map(|_| (()));
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
                     last_pushed_guid = ?5
                 WHERE id = ?1",
            )?;
            stmt.execute(params![
                &feed.uid.unwrap(),
                &feed.name,
                &feed.link,
                &channel,
                &feed.last_pushed_guid,
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
            result.push(feed.update_channel_in_place().await);
            let r = update_feed(conn, feed);
            match r {
                Ok(i) => acc += i,
                // TODO
                _ => (),
            }
        }
        for r in result {
            if let Err(e) = r {
                println!("Error in update_channels function:\n{:?}", e)
            }
        }
        Ok(acc)
    }

    pub fn get_feed(conn: &Connection, id: &u32) -> Result<Option<PubmedFeed>, rusqlite::Error> {
        let mut stmt = conn
            .prepare("SELECT id, name, link, channel, last_pushed_guid FROM feeds WHERE id=(?1)")?;
        let mut rows = stmt.query([id])?;
        let row_opt = rows.next()?;
        if let Some(row) = row_opt {
            Ok(Some(PubmedFeed {
                name: row.get(1)?,
                uid: Some(row.get(0)?),
                link: row.get(2)?,
                channel: {
                    let s: String = row.get(3)?;
                    ChannelWrapper::from_json(&s)
                        .map_err(|err| rusqlite::Error::ToSqlConversionFailure(err.into()))?
                },
                last_pushed_guid: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_feeds(conn: &Connection) -> Result<Vec<PubmedFeed>, rusqlite::Error> {
        let mut stmt =
            conn.prepare("SELECT id, name, link, channel, last_pushed_guid FROM feeds")?;
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
                    match get_feed(conn, uid)? {
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

// pub mod redis {
//     use crate::datastructs::ChannelLookupTable;
//     use crate::datastructs::User;
//     use redis::{AsyncCommands, RedisError, aio::MultiplexedConnection};

//     pub async fn connect() -> Result<MultiplexedConnection, Box<dyn std::error::Error + Sync + Send>>
//     {
//         let client = redis::Client::open("redis://127.0.0.1:7777/")?;
//         let conn = client.get_multiplexed_async_connection().await?;
//         // let conn = client.get_connection()?;
//         log::info!("Redis connection established");
//         Ok(conn)
//     }

//     pub async fn get_userlist(conn: &mut MultiplexedConnection) -> Result<Vec<User>, RedisError> {
//         conn.get::<&str, String>("userlist")
//             .await
//             .and_then(|json| Ok(serde_json::from_str::<Vec<User>>(&json).unwrap()))
//     }

//     pub async fn set_userlist(
//         conn: &mut MultiplexedConnection,
//         userlist: &Vec<User>,
//     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         conn.set::<&str, String, ()>("userlist", serde_json::to_string(userlist)?)
//             .await?;
//         Ok(())
//     }

//     pub async fn get_lookuptable(
//         conn: &mut MultiplexedConnection,
//     ) -> Result<ChannelLookupTable, RedisError> {
//         conn.get::<&str, String>("channellookuptable")
//             .await
//             .and_then(|json| Ok(serde_json::from_str::<ChannelLookupTable>(&json).unwrap()))
//     }

//     pub async fn set_lookuptable(
//         conn: &mut MultiplexedConnection,
//         lookuptable: &ChannelLookupTable,
//     ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//         conn.set::<&str, String, ()>("channellookuptable", serde_json::to_string(lookuptable)?)
//             .await?;
//         Ok(())
//     }
// }
