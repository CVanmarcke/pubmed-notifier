use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use rss::Item;
use rssnotify::commands::{AdminCommand, Command};
use rssnotify::config::Config;
use rssnotify::datastructs::{ItemMetadata, User};
use rssnotify::senders::TelegramSender;
use rssnotify::senders::{ConsoleSender, Sender};
use rssnotify::{
    admin_message_handler, console_message_handler, db, make_db, user_message_handler,
};
use std::collections::BTreeMap;
use std::env;
use std::process;
use std::sync::Arc;
use std::time::Duration;
use teloxide::Bot;
use teloxide::prelude::*;
use tokio_cron_scheduler::{JobBuilder, JobScheduler, JobSchedulerError};
use tokio_rusqlite::Connection;

#[tokio::main]
async fn main() {
    let stdout = ConsoleAppender::builder().build();

    let temp_log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))
        .unwrap();
    let logger_handle = log4rs::init_config(temp_log_config).unwrap();

    let config = Config::build_from_toml_and_args(&env::args().collect::<Vec<String>>())
        .unwrap_or_else(|err| {
            log::error!("Problem parsing arguments: {err:?}");
            process::exit(1);
        });

    log::set_max_level(config.log_level);
    let stdout = ConsoleAppender::builder().build();
    let filelogs = FileAppender::builder().build(&config.log_path).unwrap();
    let log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("logfile", Box::new(filelogs)))
        .build(
            Root::builder()
                .appender("stdout")
                .appender("logfile")
                .build(config.log_level),
        )
        .unwrap();
    logger_handle.set_config(log_config);

    config.log_structs();

    let conn = match config.db_path.is_file() {
        true => db::sqlite::open(config.db_path.to_str().unwrap()).unwrap(),
        false => make_db(&config.db_path).await.unwrap(),
    };

    let aconn = Connection::open(&config.db_path).await.unwrap();
    let arcconn = Arc::new(aconn);

    if !config.persistent {
        log::info!("Running once.");
        let r = db::sqlite::update_channels(&conn).await;
        if let Err(e) = r {
            log::error!("Error when updating the channels: {e:?}");
            process::exit(1);
        }
        if config.debugmode {
            if let Err(e) = send_new_users(&conn, &ConsoleSender {}).await {
                log::error!("Error when sending new articles: {e:?}");
            }
        } else {
            let bot = Bot::new(config.bot_token.as_ref().unwrap());
            if let Err(e) = send_new_users(&conn, &TelegramSender { bot }).await {
                log::error!("Error when sending new articles: {e:?}");
            }
        }
        process::exit(1);
    }

    if config.interactive {
        interactive_bot(&conn).await
    } else {
        let handler = Update::filter_message()
            // You can use branching to define multiple ways in which an update will be handled. If the
            // first branch fails, an update will be passed to the second branch, and so on.
            .branch(
                dptree::entry()
                    // Filter commands: the next handlers will receive a parsed `SimpleCommand`.
                    .filter_command::<Command>()
                    // If a command parsing fails, this handler will not be executed.
                    .endpoint(user_message_handler),
            )
            .branch(
                // Filter a maintainer by a user ID.
                dptree::filter(|admin: Option<u64>, msg: Message| {
                    msg.from
                        .map(|user| Some(user.id.0) == admin)
                        .unwrap_or_default()
                })
                .filter_command::<AdminCommand>()
                .endpoint(admin_message_handler),
            )
            .branch(dptree::endpoint(|bot: Bot, msg: Message| async move {
                bot.send_message(
                    msg.chat.id,
                    "Invalid command. Send /help for a list of valid commands.".to_string(),
                )
                .await?;
                Ok(())
            }));

        // if config.debugmode {
        //     scheduler(&config, Arc::clone(&arcconn), ConsoleSender {})
        //         .await
        //         .unwrap();
        // } else {
        log::info!("Starting command bot.");
        let bot = Bot::new(config.bot_token.as_ref().unwrap());
        let state = Arc::clone(&arcconn);
        // let admin = Arc::new(config.admin.clone());
        let admin = config.admin;
        tokio::task::spawn(async move {
            // Dispatcher::builder(bot, Update::filter_message().endpoint(user_message_handler))
            Dispatcher::builder(bot, handler)
                .dependencies(dptree::deps![state, admin])
                .build()
                .dispatch()
                .await
        });
        log::info!("Starting scheduler.");
        let bot = Bot::new(config.bot_token.as_ref().unwrap());
        let mut sched = scheduler(&config, arcconn, TelegramSender { bot })
            .await
            .unwrap();
        log::info!("Startup completed. Waiting for updates...");
        // }
        loop {
            let time_till = sched.time_till_next_job().await;
            match time_till {
                Ok(Some(ts)) => {
                    log::info!("Next time for job is {:?}", ts);
                    tokio::time::sleep(ts).await
                }
                _ => {
                    log::warn!("Could not get next tick for job, sleeping for 600 seconds...");
                    tokio::time::sleep(Duration::from_secs(600)).await
                }
            }
        }
    }
}

// looper(&config, &Arc::clone(&arcconn), ConsoleSender {}).await
async fn scheduler<'a, S>(
    config: &Config,
    conn: Arc<Connection>,
    sender: S,
) -> Result<JobScheduler, JobSchedulerError>
where
    S: Sender + Send + Sync + Clone + 'a + 'static,
{
    let arcconn = Arc::new(conn);
    let cron = format!("0 0 {} * * *", config.update_time);
    let sched = JobScheduler::new().await.unwrap();

    let job = JobBuilder::new()
        .with_timezone(chrono::Local)
        // .with_timezone(chrono_tz::Europe::Brussels)
        .with_cron_job_type()
        .with_schedule(cron)
        .unwrap()
        .with_run_async(Box::new(move |_uuid, mut _l| {
            let arcconn = Arc::clone(&arcconn);
            let sender = sender.clone();
            Box::pin(async move {
                if let Err(e) = arcconn
                    .call(move |conn| {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async {
                            db::sqlite::update_channels(conn)
                                .await
                                .map_err(tokio_rusqlite::Error::Rusqlite)?;
                            send_new_users(conn, &sender)
                                .await
                                .map_err(tokio_rusqlite::Error::Rusqlite)
                        })
                    })
                    .await
                {
                    log::error!("Error in the scheduler:\n{:?}", e)
                }
            })
        }))
        .build()
        .unwrap();
    sched.add(job).await?;
    sched.start().await?;
    Ok(sched)
}

async fn send_new_users<S: Sender>(
    conn: &rusqlite::Connection,
    sender: &S,
) -> Result<(), rusqlite::Error> {
    log::info!("Sending new items to all users");
    let users = db::sqlite::get_users(conn)?;
    let mut new_items: BTreeMap<u32, Vec<&Item>> = BTreeMap::new();
    let mut feeds = db::sqlite::get_feeds(conn)?;

    for feed in feeds.iter() {
        new_items.insert(feed.uid.unwrap(), feed.get_new_items_from_last());
    }

    let mut result = Vec::new();
    for user in users.iter() {
        result.push(send_new_user(conn, sender, user, &new_items).await);
        let _ = db::sqlite::update_user(conn, user);
    }

    for feed in feeds.iter_mut() {
        feed.update_guid();
    }

    db::sqlite::update_guid_feeds(conn, &feeds)?;
    for r in result {
        r?;
    }
    Ok(())
}

async fn send_new_user<S: Sender>(
    _conn: &rusqlite::Connection,
    sender: &S,
    user: &User,
    new_items: &BTreeMap<u32, Vec<&Item>>,
) -> Result<usize, rusqlite::Error> {
    for (index, collection) in user.rss_lists.iter().enumerate() {
        // TODO: add possibility to include keyword
        let item_metadata = ItemMetadata {
            collection: Some(index),
            ..Default::default()
        };
        for feed_id in collection.feeds.iter() {
            if let Some(items) = new_items.get(feed_id) {
                // Make new vec with references to the items
                let filtered: Vec<&Item> = items
                    .iter()
                    .copied()
                    .filter(|item| collection.filter_item(item))
                    .collect();
                sender.send_items(user, &filtered, &item_metadata).await;
            }
        }
    }
    Ok(1)
}

async fn interactive_bot(conn: &rusqlite::Connection) {
    // async fn interactive_bot (userdata: Arc<Mutex<Vec<User>>>, feeddata: Arc<Mutex<ChannelLookupTable>>) {
    println!("Starting interactive mode");

    println!("Enter chat_id to exec commands as");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .expect("Did not enter a correct string");
    let chat_id: i64 = line
        .trim()
        .parse()
        .expect("Invalid chat id! Was it a number?");
    line.clear();

    loop {
        println!("Enter command: ");
        std::io::stdin()
            .read_line(&mut line)
            .expect("Did not enter a correct string");
        let _ = console_message_handler(chat_id, line.trim(), conn).await;
        // let _ = console_message_handler(chat_id, &line.trim(), Arc::clone(&userdata), Arc::clone(&feeddata)).await;
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use chrono::prelude::*;

    #[test]
    fn test_date() {
        // Thu, 06 Feb 2025 06:00:00 -0500
        // let now = DateTime::to_rfc2822(&Utc::now());
        let now = Utc::now();
        let then = DateTime::parse_from_rfc2822("Thu, 06 Feb 2025 06:00:00 -0500").unwrap();
        assert!(now > then);
    }
}
