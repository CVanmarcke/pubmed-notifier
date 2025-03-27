use rss::Channel;
use rss::Item;
use std::collections::HashSet;
use std::error::Error;

// "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532453/?limit=15&name=Insights%20Imaging&utm_campaign=journals"

pub async fn get_channel(link: &str) -> Result<Channel, Box<dyn Error>> {
    let content = reqwest::get(link).await?.bytes().await?;
    let channel = Channel::read_from(&content[..])?;
    Ok(channel)
}

pub fn item_contains_keyword(item: &Item, keywords: &HashSet<String>) -> bool {
    for keyword in keywords {
        if item.content().unwrap_or("").contains(keyword)
            | item.title().unwrap_or("").to_lowercase().contains(keyword)
        {
            log::debug!("Keyword matched: {keyword}");
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::formatter::PreppedMessage;

    use super::*;
    use rss::{Channel, ItemBuilder};
    use std::fs;
    use std::fs::File;
    use std::io::BufReader;

    // #[tokio::test]
    async fn _get_feed_test() {
        let link = "https://pubmed.ncbi.nlm.nih.gov/rss/journals/101532453/?limit=5&name=Insights%20Imaging&utm_campaign=journals";
        let result = get_channel(link).await;
        assert!(result.is_ok());
        let channel = result.unwrap();

        channel.write_to(::std::io::sink()).unwrap(); // // write to the channel to a writer
        let string = channel.to_string(); // convert the channel to a string
        fs::write("channel.xml", string).expect("error writing!");
    }

    #[test]
    fn _format_test() {
        let file = File::open("channel.xml").unwrap();
        let channel = Channel::read_from(BufReader::new(file)).unwrap();
        let item = &channel.items[0];
        let content: &str = item.content().unwrap();
        println!("{}", content);
        let formatted = PreppedMessage::build(item).format_as_markdownv2();
        println!("{}", formatted);
        // assert!(false)
    }

    #[tokio::test]
    async fn whitelist_test() {
        let item = ItemBuilder::default()
            .title("Title of the item".to_string())
            .content("Content of the item.".to_string())
            .build();
        let keywords = vec!["should", "not", "contain"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let keywords2 = vec!["should", "contain", "keyword", "content"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        assert!(!item_contains_keyword(&item, &keywords));
        assert!(!item_contains_keyword(&item, &keywords2));
    }
}
