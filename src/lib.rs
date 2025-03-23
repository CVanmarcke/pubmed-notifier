use commands::message_handler;
use senders::TelegramSender;
use teloxide::types::InputFile;
use teloxide::RequestError;
use teloxide::prelude::*;
use std::io;
use std::fs;
use std::sync::Arc;
use serde::Serialize;
use serde_json;
use crate::datastructs::PubmedFeed;
use crate::datastructs::User;

pub mod rsshandler;
pub mod datastructs;
pub mod channelwrapper;
pub mod db;
pub mod commands;
pub mod preset;
pub mod config;
pub mod senders;
pub mod formatter;

#[allow(dead_code)]
pub fn write_data<T>(data: &T, path: &str) -> io::Result<()>
where T:  ?Sized + Serialize,
{
    let data = serde_json::to_string(data)?;
    fs::write(path, data)
}

pub fn load_feedlist(path: &str) ->  Result<Vec<PubmedFeed>, io::Error> {
    let data: String = fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| io::Error::other(e))
}

pub fn load_userlist(path: &str) ->  Result<Vec<User>, io::Error> {
    let data: String = fs::read_to_string(path)?;
    serde_json::from_str(&data).map_err(|e| io::Error::other(e))
}

type CustomResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub async fn repl_message_handler(bot: Bot, msg: Message, conn: Arc<tokio_rusqlite::Connection>) -> ResponseResult<()> {
// pub async fn repl_message_handler(bot: Bot, msg: Message, cmd: Command, conn: &Connection) -> Result<(),Box<dyn std::error::Error + Send + Sync>> {

    let chat_id = msg.clone().chat.id.0.clone();
    let text = String::from(msg.text().unwrap_or(""));

    let answerstring = 
        conn.call(move |conn| {
            let mut ur = db::sqlite::get_user(conn, chat_id)?;
            if ur.is_none() {
                log::info!("User {} not found, adding", chat_id);
                ur = Some(User::new(chat_id));
                db::sqlite::add_user(&conn, &ur.clone().unwrap())?;
            }

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async { 
                message_handler(&text, &mut ur.unwrap(), conn).await
                    .map_err(|e| tokio_rusqlite::Error::Other(e))
            })
        }).await
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

pub async fn console_message_handler(chat_id: i64, text: &str, conn: &rusqlite::Connection) -> CustomResult<()> {
    let ur = db::sqlite::get_user(conn, chat_id)?;
    if ur.is_none() {
        println!("User not found, adding");
        db::sqlite::add_user(conn, &User::new(chat_id))?;
    }
    match message_handler(text, &mut ur.unwrap(), conn).await {
        Ok(response) => println!("{}", response),
        Err(e) => println!("Error: {e:?}. Try /help for the list of available commands.")
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
            "Insights into Imaging"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101674571/?limit=15&name=Abdom%20Radiol%20%28NY%29&utm_campaign=journals",
            "Abdominal Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8302501/?limit=20&name=Radiographics&utm_campaign=journals",
            "Radiographics"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/7708173/?limit=15&name=AJR%20Am%20J%20Roentgenol&utm_campaign=journals",
            "American Journal of Roentgenology (AJR)"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/9114774/?limit=15&name=Eur%20Radiol&utm_campaign=journals",
            "European Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/0401260/?limit=50&name=Radiology&utm_campaign=journals",
            "Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101765309/?limit=50&name=Radiol%20Imaging%20Cancer&utm_campaign=journals",
            "Radiological Imaging Cancer"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8106411/?limit=10&name=Eur%20J%20Radiol&utm_campaign=journals",
            "European Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/100956096/?limit=20&name=Korean%20J%20Radiol&utm_campaign=journals",
            "Korean Journal of Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101490689/?limit=10&name=Jpn%20J%20Radiol&utm_campaign=journals",
            "Japanese Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8911831/?limit=10&name=Clin%20Imaging&utm_campaign=journals",
            "Clinical Imaging"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/1306016/?limit=10&name=Clin%20Radiol&utm_campaign=journals",
            "Clinical Radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101698198/?limit=10&name=J%20Belg%20Soc%20Radiol&utm_campaign=journals", "Journal of the Belgian society of radiology"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/8706123/?limit=10&name=Acta%20Radiol&utm_campaign=journals",
            "Acta Radiologica"),
        PubmedFeed::build_from_link(
            "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101721752/?limit=15&name=Eur%20Radiol%20Exp&utm_campaign=journals",
            "European Radiology Exp")];
    return feeds.into_iter().map(|x| x.unwrap()).collect();
}


#[cfg(test)]
mod tests {

    use super::*;
    use std::collections::HashSet;
    use datastructs::{ChannelLookupTable, User, UserRssList};
    use teloxide_tests::{MockBot, MockMessageText};

    #[test]
    fn test_write_userdata () {
        let path = "userdata.json";
        let mut uro_rss_list: UserRssList = UserRssList::new();
        uro_rss_list.whitelist = uro_rss_list.whitelist
            .into_iter()
            .chain(preset::uro_whitelist().into_iter())
            .collect::<HashSet<String>>();

        let mut abdomen_rss_list: UserRssList = UserRssList::new();
        abdomen_rss_list.whitelist = abdomen_rss_list.whitelist
            .into_iter()
            .chain(preset::abdomen_whitelist().into_iter())
            .collect::<HashSet<String>>();

        let user = User::build(
            1234i64,
            "31 sept 2024".to_string(),
            vec![uro_rss_list]);
        let user2 = User::build(
            12344i64,
            "31 sept 2024".to_string(),
            vec![abdomen_rss_list]);
        let userlist = vec![user, user2];
        write_data(&userlist, &path).expect("Error writing data");
    }

    // #[tokio::test]
    // async fn test_hello_world() {  // A testing bot dispatch
    //     let bot = MockBot::new(MockMessageText::new().text("/username myname"), handler_tree());
    //     bot.dispatch().await;
    //     let responses = bot.get_responses();
    //     let message = responses.sent_messages.last().unwrap();
    //     // This is a regular teloxide::types::Message!
    //     assert_eq!(message.text(), Some("Your username is @myname."));
    // }

    #[test]
    fn test_write_feedlist() {
        let list = ChannelLookupTable::from_vec(make_feedlist()).unwrap();
        let s = serde_json::to_string(&list).unwrap();
        write_data(&s, "feedlist.json").expect("Error writing data");
    }
}
