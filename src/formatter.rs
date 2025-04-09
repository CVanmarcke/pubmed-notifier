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

            let boldre = Regex::new(r"(?m)\*\*(.+?)\*\*").unwrap();
            content = boldre
                .replace_all(&content, |caps: &Captures| -> String {
                    markdown::bold(&caps[1])
                })
                .to_string();

            // For the journal "Radiology"
            let boldkeywordsre = Regex::new(r"\. (Background|Purpose|Materials and Methods|Results|Conclusion)( [A-Z])").unwrap();
            content = boldkeywordsre
                .replace_all(&content, |caps: &Captures| -> String {
                    format!(".\n\n{}:{}", markdown::bold(&caps[1].to_uppercase()), &caps[2])
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

    #[test]
    fn test_format() {
        let mut file = File::open("test/channel_abdominal_radiology.json").unwrap();
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

        assert_eq!(message, result);

        let mut file = File::open("test/channel_radiology.json").unwrap();
        let mut json = String::new();
        file.read_to_string(&mut json).unwrap();
        let channel = ChannelWrapper::from_json(&json).unwrap();

        let item = &channel.items[7];
        let message = PreppedMessage::build(item).format(ParseMode::MarkdownV2);
        let result = r"[Predicting Regional Lymph Node Metastases at CT in Microsatellite Instability\-High Colon Cancer](https://doi\.org/10\.1148/radiol\.242122)
_Radiology_

Background Early identification of lymph node metastasis is crucial for microsatellite instability\-high \(MSI\-H\) colon cancer caused by deficient mismatch repair, but accuracy of CT is poor\.

*PURPOSE*: To determine whether CT\-detected lymph node distribution patterns can improve lymph node evaluation in MSI\-H colon cancer\.

*MATERIALS AND METHODS*: This two\-center retrospective study included patients with MSI\-H colon cancer who underwent pretreatment CT and radical surgery \(development set, December 2017\-December 2022; test set, January 2016\-January 2024\)\. Lymph node characteristics associated with pathologic lymph node metastasis \(pN\+\), including clinical lymph node stage \(cN\) and distribution patterns \(vascular distribution, jammed cluster, and partial fusion\), were selected \(logistic regression and Kendall tau\-b correlation\) to create a distribution\-based clinical lymph node stage \(dcN\) in the development set\. Diagnostic performance was verified in the test set\. Interobserver agreement was assessed by using Fleiss κ\. Clinical value of dcN was assessed using univariable logistic analysis among patients in the treatment set receiving neoadjuvant immunotherapy \(August 2017\-February 2024\)\.

*RESULTS*: The study included 368 patients \(median age, 60 years \[IQR, 50\-70 years\]; 211 male\): 230 from the development set \(median age, 59 years \[IQR, 49\-70 years\]\), 86 from the test set \(median age, 66 years \[IQR, 55\-79 years\]\), and 52 from the treatment set \(median age, 54 years \[IQR, 42\-65 years\]\)\. Only jammed cluster and partial fusion were associated with higher odds of pN\+ \(odds ratio, 78\.9 and 21\.5, respectively; both\*P\*\< \.001\)\. dcN outperformed cN in the test set \(accuracy, 90% \[78 of 87\] vs 46% \[40 of 87\];\*P\*\< \.001; specificity, 97% \[55 of 57\] vs 26% \[15 of 57\];\*P\*\< \.001\)\. Interobserver agreement was moderate for dcN \(κ \= 0\.67\) and poor for cN \(κ \= 0\.48\)\. dcN was associated with a complete response after neoadjuvant immunotherapy \(odds ratio, 0\.05;\*P\*\< \.001\)\. Conclusion dcN showed high performance for identifying regional lymph node metastases and helped predict complete response after neoadjuvant immunotherapy in MSI\-H colon cancer using a surgical reference standard\. ©RSNA, 2025\*Supplemental material is available for this article\.\*See also the editorial by Lev\-Cohain and Sosna in this issue\.
[Link](https://doi\.org/10\.1148/radiol\.242122) \| [PubMed](https://pubmed\.ncbi\.nlm\.nih\.gov/40197093) \| [QxMD](https://qxmd\.com/r/40197093)";

        assert_eq!(message, result)

    }
}
