use chrono::{Local, NaiveTime, TimeDelta, Timelike};
use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use rssnotify::config::Config;
use rssnotify::datastructs::{ChannelLookupTable, User};
use rssnotify::senders::TelegramSender;
use rssnotify::senders::{ConsoleSender, Sender};
use rssnotify::{console_message_handler, db, repl_message_handler};
use std::path::Path;
use std::{env, fs};
use std::process;
use std::sync::Arc;
use std::time::Duration;
use teloxide::Bot;
use teloxide::prelude::*;
use tokio_rusqlite::Connection;

// op server in background starten
// https://www.scaler.com/topics/how-to-run-process-in-background-linux/

// https://github.com/teloxide/teloxide

// Main met maar 2 mogelijke args:
// debug (-d ofzo) en -f config.toml
// Al de rest van de config via de toml importeren...

// bij update: volledige channel in items in een lijst.
// Bij verzenden: per user, een datum van laatste verstuurd. Bij verzenden, checken obv die datum welke nieuw zijn in elke journal en dat verzenden.
// Struct per journal (algemeen, voor iedereen hetzelfde). Deze structs misschien in een dictionary obv ID (maar hashable).
// User config struct: oa last_send, telegram_id, RSS_lists (= vector met RSS_lists), update interval etc.
// RSS_list bevat een lijst van rss_queries (struct met link/id), lijst van whitelist keywords en blacklist keywords
// RSS_queries zijn gekoppeld met een ID aan journal structs.

// Telegram kan de user config structs aanpassen.
// Misschien aparte lijst per user voor laatst verzonden (zodat enkel telegram aanpassingen moet doen aan user config, geen race condition).

// Teloxide toegang geven tot struct/db met config van de user:
// https://github.com/teloxide/teloxide/discussions/471
// Aangezien de de config mee verplaatsen -> zullen in de config zelf alle data moeten steken die nodig is. evt ook save

// aparte mod voor formatter.

// Flow in main:
// 1) laad config van env en arguments
// 2) laad userinfo (in arc<mutex<>>) en lijst van feeds.
// 3) laad inhoud van feeds
// 4) start telegram bot
// 5) start async timer functie (beschreven in lib): argument: (lijst van) users, telegram bot, feeds, inhoud.
//    Deze fn kan ook refreshen?
//    Alternatief: je geeft die functie een closure die user en feeds kan aanpassen?
// 6) start async command functie (beschreven in lib): arg users, bot, feeds
//    Alternatief: je geeft die een closure die user en feeds kan aanpassen?
//    Systeem om van de command async commandos te sturen naar de andere?

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

    log::set_max_level(config.log_level.clone());
    let stdout = ConsoleAppender::builder().build();
    let filelogs = FileAppender::builder()
        .build(&config.log_path)
        .unwrap();
    let log_config = log4rs::Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .appender(Appender::builder().build("logfile", Box::new(filelogs)))
        // .logger(Logger::builder()
        //     .appender("logfile")
        //     .additive(false)
        //     .build("filelogger", LevelFilter::Info))
        .build(Root::builder()
            .appender("stdout")
            .appender("logfile").build(LevelFilter::Info))
        .unwrap();
    logger_handle.set_config(log_config);

    config.log_structs();

    let conn;
    if config.db_path.is_file() {
        conn = db::sqlite::open(config.db_path.to_str().unwrap()).unwrap();
    } else {
        log::info!("Creating new database at {}", &config.db_path.display());
        fs::create_dir_all(config.db_path.parent().unwrap_or(Path::new(""))).unwrap();
        conn = db::sqlite::new(&config.db_path.display().to_string()).unwrap();
        let r = db::sqlite::update_channels(&conn).await;
        if r.is_err() {
            log::error!("Error when updating the channel!\n{:?}", r.err().unwrap())
        }
    }

    let aconn = Connection::open(config.db_path.to_str().unwrap())
        .await
        .unwrap();
    let arcconn = Arc::new(aconn);

    if !config.persistent {
        // TODO
        let r = db::sqlite::update_channels(&conn).await;
        if let Err(e) = r {
            log::error!("Error when updating the channels: {e:?}");
            process::exit(1);
        }
        if config.debugmode {
            if let Err(e) = send_new_and_update_users(&conn, &ConsoleSender {}).await {
                log::error!("Error when sending new articles: {e:?}");
            }
        } else {
            let bot = Bot::new(config.bot_token.as_ref().unwrap());
            if let Err(e) = send_new_and_update_users(&conn, &TelegramSender { bot }).await {
                log::error!("Error when sending new articles: {e:?}");
            }
            
        }
        process::exit(1);
    }

    if config.interactive {
        interactive_bot(&conn).await
    } else {
        if config.debugmode {
            looper(&config, &Arc::clone(&arcconn), ConsoleSender {}).await
        } else {
            log::info!("Starting command bot");
            let bot = Bot::new(config.bot_token.as_ref().unwrap());

            let state = Arc::clone(&arcconn);
            // Start dispatcher as a seperate task
            // https://users.rust-lang.org/t/dependency-injection-callback-telegram-bot/88131
            tokio::task::spawn(async {
                Dispatcher::builder(bot, Update::filter_message().endpoint(repl_message_handler))
                    .dependencies(dptree::deps![state])
                    .build()
                    .dispatch()
                    .await
            });
            log::info!("Command bot started");

            let bot = Bot::new(config.bot_token.as_ref().unwrap());
            log::info!("Starting scheduler");
            looper(
                &config,
                &Arc::clone(&arcconn),
                TelegramSender { bot },
            )
            .await
        }
    }
}

pub async fn looper<'a, S>(config: &Config, conn: &'a Connection, sender: S) -> ()
where
    S: Sender + Send + Sync + 'a + 'static,
{
    let times: &Vec<NaiveTime> = &(config.update_time);

    let mut execute_next = false;
    let max_wait_time = 5 * 60;
    let senderarc = Arc::new(sender);
    loop {
        let now = Local::now().naive_local().time();
        let wait_time: u64;
        let duration = times
            .iter()
            .map(|dt| dt.signed_duration_since(now))
            .filter(|dt| dt > &TimeDelta::zero())
            .min();

        if duration.is_none() {
            // of max wait time
            wait_time = (86399 - now.num_seconds_from_midnight()).into();
        } else {
            // is always positive as we filtered that in de duration map
            wait_time = u64::try_from(duration.unwrap().num_seconds()).unwrap();
        }
        if wait_time < max_wait_time {
            execute_next = true;
        }
        tokio::time::sleep(Duration::from_secs(wait_time)).await;
        let asender = Arc::clone(&senderarc);
        if execute_next {
            if let Err(e) = conn
                .call(move |conn| {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        db::sqlite::update_channels(conn).await?;
                        send_new_and_update_users(conn, &(*asender)).await
                    })?;
                    Ok(())
                })
                .await
            {
                log::error!("Error updating the channels: {e:?}")
            }
        }
        execute_next = false;
    }
}

async fn send_new_and_update_users<S: Sender>(
    conn: &rusqlite::Connection,
    sender: &S,
) -> Result<(), rusqlite::Error> {
    let mut users = db::sqlite::get_users(conn)?;
    let feeds = ChannelLookupTable::from_vec(db::sqlite::get_feeds(conn)?).map_err(|e| {
        rusqlite::Error::InvalidParameterName(format!("Error when updating tables: {e:?}"))
    })?;
    let mut result = Vec::new();
    for user in users.iter_mut() {
        result.push(send_new_and_update_user(conn, sender, user, &feeds).await)

    }
    for r in result {
        r?;
    }
    Ok(())
}

async fn send_new_and_update_user<S: Sender>(
    conn: &rusqlite::Connection,
    sender: &S,
    user: &mut User,
    feeds: &ChannelLookupTable
) -> Result<usize, rusqlite::Error> {
    if let Some(items) = user.get_new_items(&feeds) {
        log::info!(
            "{} new items for user {} to send",
            items.len(),
            user.chat_id
        );
        // TODO result handling
        sender.send_items(user, &items).await;
        user.update_last_pushed();
        db::sqlite::update_user(conn, &user)
    } else {
        Ok(0)
    }
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
        let _ = console_message_handler(chat_id, &line.trim(), &conn).await;
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


