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
            item.dublin_core_ext()
                .unwrap()
                .clone()
                .sources()
                .iter()
                .next()
                .unwrap()
                .to_owned(),
        );

        let content_formatted = html2md::rewrite_html(item.content().unwrap_or(""), false);
        // .replace("**", "*");
        log::debug!("{}", content_formatted);

        let abstr_start = content_formatted.find("**ABSTRACT**\n");
        let pmid_start = content_formatted.find("PMID:[").unwrap_or(0);
        if abstr_start.is_some() && pmid_start > 0 {
            content = Some(
                content_formatted[abstr_start.unwrap() + 13..pmid_start]
                    .trim()
                    .to_string(),
            );
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
            title,
            journal,
            content,
            pmid,
            doi,
        }
    }

    fn format_link_markdownv2(text: &str, baseurl: &str, pmid_or_doi: &str) -> String {
        markdown::link(
            &markdown::escape(&format!("{}{}", baseurl, pmid_or_doi)),
            &markdown::escape(text),
        )
    }

    fn format_as_markdownv2(&self) -> String {
        let mut result = "".to_string();
        let mut footer;
        if let Some(doi) = &self.doi {
            result.push_str(&PreppedMessage::format_link_markdownv2(
                &self.title,
                "https://doi.org/",
                doi,
            ));
            result.push_str("\n");
            if let Some(journal) = &self.journal {
                result.push_str(&markdown::italic(&markdown::escape(journal)));
            }
            if let Some(content) = &self.content {
                result.push_str("\n\n");
                result.push_str(&PreppedMessage::format_abstract(
                    &content,
                    ParseMode::MarkdownV2,
                ));
            }
            footer = PreppedMessage::format_link_markdownv2("Link", "https://doi.org/", doi);

            if let Some(pmid) = &self.pmid {
                footer.push_str(&format!(
                    " \\| {} \\| {}",
                    &PreppedMessage::format_link_markdownv2(
                        "PubMed",
                        "https://pubmed.ncbi.nlm.nih.gov/",
                        pmid
                    ),
                    &PreppedMessage::format_link_markdownv2("QxMD", "https://qxmd.com/r/", pmid)
                ));
            }
            result.push_str("\n");
            result.push_str(&footer);
            log::debug!("{}", result);
            result
        } else {
            result.push_str(&markdown::escape(&self.title));
            if let Some(journal) = &self.journal {
                result.push_str(&markdown::italic(&markdown::escape(journal)));
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
            content = content.replace(r"&lt;", r"\<");
            content = content.replace(r"&gt;", r"\>");

            // TODO TESTEN
            let boldre = Regex::new(r"(?m)\*\*(.+?)\*\*").unwrap();
            content = boldre
                .replace_all(&content, |caps: &Captures| -> String {
                    markdown::bold(&caps[1])
                })
                .to_string();

            let re = Regex::new(r"(?m)^([A-Z ]+:) ").unwrap();
            re.replace_all(&content, |caps: &Captures| -> String {
                format!("\n{} ", markdown::bold(&caps[1]))
            })
            .trim()
            .to_string()
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

    let abstr_start = content.find("*ABSTRACT*\n");
    let pmid_start = content.find("PMID:[").unwrap();
    if let Some(x) = abstr_start {
        content = content[x + 11..pmid_start].to_string();
    } else {
        return "No abstract.".to_string();
    }

    if source.is_some() {
        content = format!("_{}_\n\n{}", source.unwrap(), content);
    }
    if doi.is_some() {
        title = format!("[{}](https://doi.org/{})", title, doi.unwrap());
        content = format!("{}\n[link](https://doi.org/{}) | ", content, doi.unwrap());
    }
    if pmid.is_some() {
        content = format!(
            "{}[Pubmed](https://pubmed.ncbi.nlm.nih.gov/{}/) | \
[QxMD](https://qxmd.com/r/{})",
            content,
            pmid.unwrap(),
            pmid.unwrap()
        );
    }
    return format!("{title}\n{content}");
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read};

    use crate::channelwrapper::ChannelWrapper;

    use super::*;
    // use rssnotify::db::sqlite::*;

    #[test]
    fn test_format() {
        let mut file = File::open("test/samplechannel.xml").unwrap();
        let mut json = String::new();
        file.read_to_string(&mut json).unwrap();
        let channel = ChannelWrapper::from_json(&json).unwrap();

        let item = &channel.items[0];
        let message = PreppedMessage::build(item).format(ParseMode::MarkdownV2);
        let result = r"[Deep learning assisted detection and segmentation of uterine fibroids using multi\-orientation magnetic resonance imaging](https://doi\.org/10\.1007/s00261\-025\-04934\-8)
_Abdominal radiology \(New York\)_

*PURPOSE:* To develop deep learning models for automated detection and segmentation of uterine fibroids using multi\-orientation MRI\.

*METHODS:* Pre\-treatment sagittal and axial T2\-weighted MRI scans acquired from patients diagnosed with uterine fibroids were collected\. The proposed segmentation models were constructed based on the three\-dimensional nnU\-Net framework\. Fibroid detection efficacy was assessed, with subgroup analyses by size and location\. The segmentation performance was evaluated using Dice similarity coefficients \(DSCs\), 95% Hausdorff distance \(HD95\), and average surface distance \(ASD\)\.

*RESULTS:* The internal dataset comprised 299 patients who were divided into the training set \(n \= 239\) and the internal test set \(n \= 60\)\. The external dataset comprised 45 patients\. The sagittal T2WI model and the axial T2WI model demonstrated recalls of 74\.4%/76\.4% and precision of 98\.9%/97\.9% for fibroid detection in the internal test set\. The models achieved recalls of 93\.7%/95\.3% for fibroids ≥4 cm\. The recalls for International Federation of Gynecology and Obstetrics \(FIGO\) type 2\-5, FIGO types 0\\1\\2\(submucous\), fibroids FIGO types 5\\6\\7\(subserous\) were 100%/100%, 73\.3%/78\.6%, and 80\.3%/81\.9%, respectively\. The proposed models demonstrated good performance in segmentation of the uterine fibroids with mean DSCs of 0\.789 and 0\.804, HD95s of 9\.996 and 10\.855 mm, and ASDs of 2\.035 and 2\.115 mm in the internal test set, and with mean DSCs of 0\.834 and 0\.818, HD95s of 9\.971 and 11\.874 mm, and ASDs of 2\.031 and 2\.273 mm in the external test set\.

*CONCLUSION:* The proposed deep learning models showed promise as reliable methods for automating the detection and segmentation of the uterine fibroids, particularly those of clinical relevance\.
[Link](https://doi\.org/10\.1007/s00261\-025\-04934\-8) \| [PubMed](https://pubmed\.ncbi\.nlm\.nih\.gov/40188260) \| [QxMD](https://qxmd\.com/r/40188260)";

        assert_eq!(message, result)
    }
}
