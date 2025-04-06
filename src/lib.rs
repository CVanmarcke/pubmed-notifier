use crate::datastructs::PubmedFeed;
use crate::datastructs::User;
use commands::message_handler;
use senders::TelegramSender;
use serde::Serialize;
use serde_json;
use std::fs;
use std::io;
use std::sync::Arc;
use teloxide::RequestError;
use teloxide::prelude::*;
use teloxide::types::InputFile;

pub mod channelwrapper;
pub mod commands;
pub mod config;
pub mod datastructs;
pub mod db;
pub mod formatter;
pub mod preset;
pub mod rsshandler;
pub mod senders;

#[allow(dead_code)]
pub fn write_data<T>(data: &T, path: &str) -> io::Result<()>
where
    T: ?Sized + Serialize,
{
    let data = serde_json::to_string(data)?;
    fs::write(path, data)
}

pub fn load_feedlist(path: &str) -> Result<Vec<PubmedFeed>, io::Error> {
    let data: String = fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| io::Error::other(e))
}

pub fn load_userlist(path: &str) -> Result<Vec<User>, io::Error> {
    let data: String = fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| io::Error::other(e))
}

type CustomResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub async fn repl_message_handler(
    bot: Bot,
    msg: Message,
    conn: Arc<tokio_rusqlite::Connection>,
) -> ResponseResult<()> {
    // pub async fn repl_message_handler(bot: Bot, msg: Message, cmd: Command, conn: &Connection) -> Result<(),Box<dyn std::error::Error + Send + Sync>> {

    let chat_id = msg.clone().chat.id.0.clone();
    let text = String::from(msg.text().unwrap_or(""));

    let answerstring = conn
        .call(move |conn| {
            let mut ur = db::sqlite::get_user(conn, chat_id)?;
            if ur.is_none() {
                log::info!("User {} not found, adding", chat_id);
                ur = Some(User::new(chat_id));
                db::sqlite::add_user(&conn, &ur.clone().unwrap())?;
            }

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                message_handler(&text, &mut ur.unwrap(), conn)
                    .await
                    .map_err(|e| tokio_rusqlite::Error::Other(e))
            })
        })
        .await
        .map_err(|e| RequestError::Io(std::io::Error::other(format!("{e:?}"))))?;

    if answerstring.len() > 4000 {
        let document = InputFile::memory(answerstring).file_name("reply.txt");
        bot.send_document(msg.chat.id, document)
            .caption("Answer is provided in the file as it was too long.")
            .send()
            .await?;
    } else {
        TelegramSender::send_message_bot(&bot, msg.chat.id, &answerstring).await?;
    }
    Ok(())
}

pub async fn console_message_handler(
    chat_id: i64,
    text: &str,
    conn: &rusqlite::Connection,
) -> CustomResult<()> {
    let mut ur = db::sqlite::get_user(conn, chat_id)?;
    if ur.is_none() {
        println!("User not found, adding");
        ur = Some(User::new(chat_id));
        db::sqlite::add_user(conn, &ur.as_ref().unwrap())?;
    }
    match message_handler(text, &mut ur.unwrap(), conn).await {
        Ok(response) => println!("{}", response),
        Err(e) => println!("Error: {e:?}. Try /help for the list of available commands."),
    }
    Ok(())
}

// https://docs.rs/teloxide/latest/teloxide/dispatching/type.UpdateHandler.html
// https://docs.rs/dptree/0.3.0/dptree/index.html
// fn handler_tree() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
//     // A simple handler. But you need to make it into a separate thing!
//     // dptree::entry().branch(Update::filter_message().endpoint(hello_world))
//     dptree::entry().branch(Update::filter_message().endpoint(bot_message_handler))
//         //TEST
// }

// pub async fn command_bot(bot: Bot, userdata: Arc<Mutex<Vec<User>>>, feeddata: Arc<Mutex<ChannelLookupTable>>) -> () {  // A regular bot dispatch
//     Dispatcher::builder(bot, handler_tree())
//         .enable_ctrlc_handler()
//         .build()
//         .dispatch()
//         .await;
// }

pub fn make_feedlist() -> Vec<PubmedFeed> {
    let feeds = vec![
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532453/?limit=15&name=Insights%20Imaging&utm_campaign=journals",
            "Insights into Imaging",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101674571/?limit=15&name=Abdom%20Radiol%20%28NY%29&utm_campaign=journals",
            "Abdominal Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8302501/?limit=20&name=Radiographics&utm_campaign=journals",
            "Radiographics",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/7708173/?limit=15&name=AJR%20Am%20J%20Roentgenol&utm_campaign=journals",
            "American Journal of Roentgenology (AJR)",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/9114774/?limit=15&name=Eur%20Radiol&utm_campaign=journals",
            "European Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/0401260/?limit=50&name=Radiology&utm_campaign=journals",
            "Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101765309/?limit=50&name=Radiol%20Imaging%20Cancer&utm_campaign=journals",
            "Radiological Imaging Cancer",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8106411/?limit=10&name=Eur%20J%20Radiol&utm_campaign=journals",
            "European Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/100956096/?limit=20&name=Korean%20J%20Radiol&utm_campaign=journals",
            "Korean Journal of Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101490689/?limit=10&name=Jpn%20J%20Radiol&utm_campaign=journals",
            "Japanese Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8911831/?limit=10&name=Clin%20Imaging&utm_campaign=journals",
            "Clinical Imaging",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/1306016/?limit=10&name=Clin%20Radiol&utm_campaign=journals",
            "Clinical Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101698198/?limit=10&name=J%20Belg%20Soc%20Radiol&utm_campaign=journals",
            "Journal of the Belgian Society of Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8706123/?limit=10&name=Acta%20Radiol&utm_campaign=journals",
            "Acta Radiologica",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101721752/?limit=15&name=Eur%20Radiol%20Exp&utm_campaign=journals",
            "European Radiology Exp",
        ),


        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8505245/?limit=10&name=Magn%20Reson%20Med&utm_campaign=journals",
            "Magnetic Resonance in Medicine",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/9105850/?limit=10&name=J%20Magn%20Reson%20Imaging&utm_campaign=journals",
            "Journal of Magnetic Resonance Imaging",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/9707935/?limit=10&name=J%20Magn%20Reson&utm_campaign=journals",
            "Journal of Magnetic Resonance",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/7703942/?limit=10&name=J%20Comput%20Assist%20Tomogr&utm_campaign=journals",
            "Journal of Computer Assisted Tomography",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/9440159/?limit=15&name=Acad%20Radiol&utm_campaign=journals",
            "Academic Radiology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8211547/?limit=10&name=J%20Ultrasound%20Med&utm_campaign=journals",
            "Journal of Ultrasound in Medicine",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101626019/?limit=15&name=Ultrasonography&utm_campaign=journals",
            "Ultrasonography",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101315005/?limit=10&name=J%20Ultrasound&utm_campaign=journals",
            "Journal of Ultrasound",
        ),

        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/0374630/?limit=10&name=Gastroenterology&utm_campaign=journals",
            "Gastroenterology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/100966936/?limit=10&name=Pancreatology&utm_campaign=journals",
            "Pancreaticology",
        ),


        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/7512719/?limit=15&name=Eur%20Urol&utm_campaign=journals",
            "European Urology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/0376374/?limit=15&name=J%20Urol&utm_campaign=journals",
            "Journal of Urology",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101724904/?limit=10&name=Eur%20Urol%20Oncol&utm_campaign=journals",
            "European Urology Oncology",
        ),


        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/100909747/?limit=10&name=Cochrane%20Database%20Syst%20Rev&utm_campaign=journals",
            "Cochrane Database of Systematic Reviews",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101589553/?limit=15&name=JAMA%20Surg&utm_campaign=journals",
            "JAMA surgery",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/2985213R/?limit=10&name=Lancet&utm_campaign=journals",
            "Lancet",
        ),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/0255562/?limit=20&name=N%20Engl%20J%20Med&utm_campaign=journals",
            "NEJM",
        ),


    ];
    return feeds.into_iter().map(|x| x.unwrap()).collect();
}

#[cfg(test)]
mod tests {

    use super::*;
    use chrono::{DateTime, Local, TimeDelta};
    use datastructs::{ChannelLookupTable, User, UserRssList};
    use formatter::PreppedMessage;
    use preset::{Journals, Keywords};
    use rsshandler::item_contains_keyword;

    #[test]
    fn test_write_userdata() {
        let path = "userdata.json";
        let mut uro_rss_list: UserRssList = UserRssList::new();

        uro_rss_list.whitelist = preset::merge_keyword_preset_with_set(Keywords::Uro, &uro_rss_list.whitelist);


        let mut abdomen_rss_list: UserRssList = UserRssList::new();
        abdomen_rss_list.whitelist = preset::merge_keyword_preset_with_set(Keywords::Abdomen, &abdomen_rss_list.whitelist);

        let user = User::build(1234i64, "31 sept 2024".to_string(), vec![uro_rss_list]);
        let user2 = User::build(12344i64, "31 sept 2024".to_string(), vec![abdomen_rss_list]);
        let userlist = vec![user, user2];
        write_data(&userlist, &path).expect("Error writing data");
    }

    #[test]
    fn test_write_feedlist() {
        let list = ChannelLookupTable::from_vec(make_feedlist()).unwrap();
        let s = serde_json::to_string(&list).unwrap();
        write_data(&s, "feedlist.json").expect("Error writing data");
    }

    #[tokio::test]
    async fn test_whitelist() {
        let mut collection = UserRssList::new();
        collection.whitelist = preset::merge_keyword_preset_with_set(Keywords::Uro, &collection.whitelist);
        collection.whitelist = preset::merge_keyword_preset_with_set(Keywords::Abdomen, &collection.whitelist);
        collection.blacklist = preset::merge_keyword_preset_with_set(Keywords::DefaultBlacklist, &collection.blacklist);
        collection.blacklist = preset::merge_keyword_preset_with_set(Keywords::AIBlacklist, &collection.blacklist);
        collection.feeds =
preset::merge_journal_preset_with_set(Journals::Radiology, &collection.feeds);
        let last_pushed = Local::now() - TimeDelta::days(3);
        println!("printing from {}", last_pushed);

        let conn = db::sqlite::open("database.db3").unwrap();
        // db::sqlite::update_channels(&conn).await;

        let mut to_send = Vec::new();
        for uid in &collection.feeds {
            let pmfeed = db::sqlite::get_feed(&conn, uid).unwrap().unwrap();
            let mut items = Vec::new();
            for item in pmfeed.channel.items() {
                if let Some(pub_date) = item.pub_date() {
                    if DateTime::parse_from_rfc2822(&pub_date).unwrap() > last_pushed {
                        items.push(item);
                    }
                }
            }
            items
                .into_iter()
                .inspect(|item| {
                    println!("--------------------------------------");
                    println!("{}", PreppedMessage::build(item)
                        .format(teloxide::types::ParseMode::MarkdownV2));
                })
                .filter(|item| item_contains_keyword(item, &collection.whitelist))
                .inspect(|_| println!("- Passed whitelist"))
                .filter(|item| !item_contains_keyword(item, &collection.blacklist))
                .inspect(|_| println!("- Passed blacklist"))
                .for_each(|item| to_send.push(item.clone()))
        }
    }
}
