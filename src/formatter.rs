use regex::{Captures, Regex};
use rss::Item;
use teloxide::{types::ParseMode, utils::markdown};

pub struct PreppedMessage {
    pub title: String,
    pub journal: Option<String>,
    pub content: Option<String>,
    pub pmid: Option<String>,
    pub doi: Option<String>,
}

impl PreppedMessage {
    pub fn build(item: &Item) -> PreppedMessage {
        let title = html2md::rewrite_html(item.title().unwrap_or(""), false);
        let mut content = None;
        let journal = Some(
            item.dublin_core_ext().unwrap().clone()
                .sources().iter()
                .next().unwrap().to_owned());

        let content_formatted = html2md::rewrite_html(item.content().unwrap_or(""), false).replace("**", "*");
        log::debug!("{}", content_formatted);

        let abstr_start = content_formatted.find("*ABSTRACT*\n");
        let pmid_start = content_formatted.find("PMID:[").unwrap_or(0);
        if abstr_start.is_some() && pmid_start > 0 {
            content = Some(content_formatted[abstr_start.unwrap() + 11 ..pmid_start].to_string());
        }        

        let (mut pmid, mut doi) = (None, None);
        let identifiers = item.dublin_core_ext().unwrap().identifiers();
        for id in identifiers {
            if id.contains("pmid:") {
                pmid = Some(id[5..].to_string());
            } else if id.contains("doi:") {
                doi = Some(id[4..].to_string());
            }
        }
        PreppedMessage {
            title, journal, content, pmid, doi
        }
    }

    fn format_link_markdownv2(text: &str, baseurl: &str, pmid_or_doi: &str) -> String {
        markdown::link(
            &markdown::escape(
                &format!("{}{}", baseurl, pmid_or_doi)),
            &markdown::escape(text))
    }

    pub fn format_as_markdownv2(&self) -> String {
        let mut result = "".to_string();
        let mut footer;
        if let Some(doi) = &self.doi {
            result.push_str(
                &PreppedMessage::format_link_markdownv2(&self.title, "https://doi.org/", doi));
            result.push_str("\n");
            if let Some(journal) = &self.journal {
                result.push_str(
                    &markdown::italic(&markdown::escape(journal)));
            }
            if let Some(content) = &self.content {
                result.push_str("\n\n");
                result.push_str(&PreppedMessage::format_abstract(&content, ParseMode::MarkdownV2));
            }
            footer = 
            PreppedMessage::format_link_markdownv2("Link", "https://doi.org/", doi);

            if let Some(pmid) = &self.pmid {
                footer.push_str(
                    &format!(
                        " \\| {} \\| {}",
                        &PreppedMessage::format_link_markdownv2("PubMed", "https://pubmed.ncbi.nlm.nih.gov/", pmid),
                        &PreppedMessage::format_link_markdownv2("QxMD", "https://qxmd.com/r/", pmid)));
            }
            result.push_str("\n");
            result.push_str(&footer);
            log::debug!("{}", result);
            result
        } else {
            result.push_str(&markdown::escape(&self.title));
            if let Some(journal) = &self.journal {
                result.push_str(
                    &markdown::italic(&markdown::escape(journal)));
            }
            if let Some(content) = &self.content {
                result.push_str("\n\n");
                result.push_str(&markdown::escape(&content));
            }
            log::info!("{}", result);
            result
        }
    }

    pub fn format(&self, parsemode: ParseMode) -> String {
        match parsemode {
            ParseMode::MarkdownV2 => self.format_as_markdownv2(),
            _ => panic!(),
        }
    }

    fn format_abstract(content: &str, parsemode: ParseMode) -> String {
        // Formats the abstract (escapes invalid characters, bolds RESULT: etc) 
        if parsemode == ParseMode::MarkdownV2 {
            let mut content = markdown::escape(content);
            content = content.replace(r"&lt;", "<");
            content = content.replace(r"&gt;", ">");

            let re = Regex::new(r"(?m)^[A-Z ]+:").unwrap();
            re.replace_all(&content, |caps: &Captures| -> String {
                markdown::bold(&caps[0])
            }).to_string()
        } else {
            todo!()
        }
        
    }
}



pub fn format_item_content(item: &Item) -> String {
    let mut title = html2md::rewrite_html(item.title().unwrap_or(""), false);
    let mut content = html2md::rewrite_html(item.content().unwrap_or(""), false);
    content = content.replace("**", "*");

    let (mut pmid, mut doi) = (None, None);
    let identifiers = item.dublin_core_ext().unwrap().identifiers();
    for id in identifiers {
        if id.contains("pmid:") {
            pmid = Some(&id[5..]);
        } else if id.contains("doi:") {
            doi = Some(&id[4..]);
        }
    }
    let source = item.dublin_core_ext().unwrap().sources().iter().next();

    
    // # Adds newlines to **Conclusion:**
    // content = re.sub(r'(\w\.)(?: |\n)(\*\*\w)', r'\1\n\n\2', content)
    // content = re.sub(r'(\w[),:\w])\n(\(?\[?\w)', #fix newlines
    //                  r'\1 \2',
    //                  content)
    // content = re.sub(r'([ -]\d)\n(\(?\[?\w)', #fix newlines with numbers
    //                  r'\1 \2',
    //                  content)
    // # removes too many newlines in certain places when wordwith-\nhyphen
    // content = re.sub(r'(\w-)\n(\w)', r'\1\2', content)
    // # If eg "KEY POINTS:"  occurs in the middle of the text, add some new lines before it
    // content = re.sub(r'\. ([A-Z ]{7,}: )', r'.\n\n\1', content)
    let abstr_start = content.find("*ABSTRACT*\n");
    let pmid_start = content.find("PMID:[").unwrap();
    if let Some(x) = abstr_start {
        content = content[x + 11 ..pmid_start].to_string();
    } else {
        return "No abstract.".to_string()
    }

    if source.is_some() {
        content = format!(
            "_{}_\n\n{}", source.unwrap(), content);
    }
    if doi.is_some() {
        title = format!("[{}](https://doi.org/{})", title, doi.unwrap());
        content = format!(
            "{}\n[link](https://doi.org/{}) | ", content, doi.unwrap());
    }
    if pmid.is_some() {
        content = format!(
            "{}[Pubmed](https://pubmed.ncbi.nlm.nih.gov/{}/) | \
[QxMD](https://qxmd.com/r/{})", content, pmid.unwrap(), pmid.unwrap());
    }
    return format!("{title}\n{content}");
}



