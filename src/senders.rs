use rss::Item;
use teloxide::utils::markdown;
use std::error::Error;
use teloxide::RequestError;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::Requester;
use teloxide::types::{LinkPreviewOptions, Message, ParseMode};
use teloxide::{Bot, types::ChatId};

use crate::datastructs::User;
use crate::formatter::PreppedMessage;

pub trait Sender {
    async fn send_item(&self, user: &User, item: &Item)
    -> Result<(), Box<dyn Error + Sync + Send>>;
    async fn send_items(
        &self,
        user: &User,
        items: &Vec<&Item>,
    ) -> Vec<Result<(), Box<dyn Error + Sync + Send>>>;
}

#[derive(Copy, Clone, Debug)]
pub struct ConsoleSender;

impl ConsoleSender {
    pub fn new() -> ConsoleSender {
        ConsoleSender {}
    }
}

impl Sender for ConsoleSender {
    async fn send_item(
        &self,
        user: &User,
        item: &Item,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        println!("----------------------------------------------");
        println!("Sendin the following item to userid {}", user.chat_id);
        println!(
            "Last_update: {}",
            item.pub_date().unwrap_or("Last update field was empty!")
        );
        println!("{}", PreppedMessage::build(item).format_as_markdownv2());

        Ok(())
    }

    async fn send_items(
        &self,
        user: &User,
        items: &Vec<&Item>,
    ) -> Vec<Result<(), Box<dyn Error + Sync + Send>>> {
        log::info!(
            "Sending {} items to the console for user {}",
            items.len(),
            user.chat_id
        );
        let mut r = Vec::new();
        for item in items {
            r.push(self.send_item(user, item).await);
        }
        r
    }
}

#[derive(Debug, Clone)]
pub struct TelegramSender {
    pub bot: Bot,
}
impl TelegramSender {
    const PREVIEW: LinkPreviewOptions = LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    };

    pub fn new(bot: Bot) -> TelegramSender {
        TelegramSender { bot }
    }

    pub async fn send_message(
        &self,
        chat_id: ChatId,
        message: &str,
    ) -> Result<Message, RequestError> {
        Self::send(&self.bot, chat_id, message).await
    }

    pub async fn send_message_bot(
        bot: &Bot,
        chat_id: ChatId,
        message: &str,
    ) -> Result<Message, RequestError> {
        // IMPORTANT: needs to be cleaned!!
        Self::send(bot, chat_id, &markdown::escape(message)).await
    }

    pub async fn send(
        bot: &Bot,
        chat_id: ChatId,
        message: &str,
    ) -> Result<Message, RequestError> {
        // IMPORTANT: needs to be cleaned!!
        bot.send_message(chat_id, message)
            .parse_mode(ParseMode::MarkdownV2)
            .link_preview_options(TelegramSender::PREVIEW)
            .await
    }

}

impl Sender for TelegramSender {
    async fn send_item(
        &self,
        user: &User,
        item: &Item,
    ) -> Result<(), Box<dyn Error + Sync + Send>> {
        if item.content().is_some() {
            let formatted = PreppedMessage::build(item).format_as_markdownv2();
            log::trace!("Sending the following item to userid {}", user.chat_id);
            log::trace!("{}", formatted);
            let result = self.send_message(ChatId(user.chat_id), &formatted).await;
            if let Err(e) = result {
                log::error!("Error when sending an item: {e:?}");
                Err(e)?;
            }
            Ok(())
        } else {
            // TODO titel ofzo ook mee in de warning
            log::warn!("Item did not have content!");
            Err("Item did not have content".into())
        }
    }

    async fn send_items(
        &self,
        user: &User,
        items: &Vec<&Item>,
    ) -> Vec<Result<(), Box<dyn Error + Sync + Send>>> {
        let mut r = Vec::new();
        for item in items {
            r.push(self.send_item(user, item).await);
        }
        // TODO joinset want toch allemaal futures....
        r
    }
}
