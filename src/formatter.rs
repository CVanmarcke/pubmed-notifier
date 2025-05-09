use regex::{Captures, Regex};
use rss::Item;
use std::{borrow::Cow, sync::LazyLock};
use teloxide::{types::ParseMode, utils::markdown};

pub struct PreppedMessage {
    pub title: String,
    pub journal: Option<String>,
    pub content: Option<String>,
    pub pmid: Option<String>,
    pub doi: Option<String>,
}

// We make a lazyLock of a struct with our compiled regex queries.
// That way we don'nt need to recompile the regex query every time.
// https://doc.rust-lang.org/std/sync/struct.LazyLock.html
static REGEXSTRUCT: LazyLock<RegexStruct> = LazyLock::new(|| RegexStruct::new());

enum RegexFilter {
    RemoveItalicKeyword,
    BoldKeyword,
    CapitalizeKeyword,
    Bold,
    Italic,
}

struct RegexStruct {
    pub remove_italic_keyword_re: Regex,
    pub bold_keyword_re: Regex,
    pub capital_keyword_re: Regex,
    pub bold_re: Regex,
    pub italic_re: Regex,
}

impl RegexStruct {
    fn new() -> RegexStruct {
        log::debug!("Initializing RegexStruct. This should only happen once.");
        RegexStruct {
            remove_italic_keyword_re: Regex::new(r"(?m)(^|\w|\.)\\_\\_([A-Za-z ]+?)[:.]\\_\\_(\w)").unwrap(),
            bold_keyword_re: Regex::new(r"(\.|^) ?(Background|Objective|Purpose|Materials and Methods|Results|Conclusion|Clinical Impact|Evidence Synthesis|Evidence Acquisition)[:.]? ?([A-Z])").unwrap(),
            capital_keyword_re: Regex::new(r"(?m)(^|\.) ?([A-Z ]+:) ").unwrap(),
            bold_re: Regex::new(r"(?m)\*\*(.+?)\*\*").unwrap(),
            italic_re: Regex::new(r"(?m)\*(.+?)\*").unwrap(),
        }
    }
    pub fn apply<'a>(&self, text: &'a str, filter: RegexFilter) -> Cow<'a, str> {
        match filter {
            RegexFilter::RemoveItalicKeyword => self
                .remove_italic_keyword_re
                .replace_all(text, |caps: &Captures| -> String {
                    format!("{} {}: {}", &caps[1], &caps[2], &caps[3])
                }),
            RegexFilter::BoldKeyword => self
                .bold_keyword_re // DONT FORGET TO TRIM
                .replace_all(text, |caps: &Captures| -> String {
                    format!(
                        "{}\n\n{} {}",
                        &caps[1],
                        markdown::bold(&(caps[2].to_uppercase() + ":")),
                        &caps[3]
                    )
                }),
            RegexFilter::CapitalizeKeyword => {
                self.capital_keyword_re
                    .replace_all(text, |caps: &Captures| -> String {
                        // DONT FORGET TO TRIM
                        format!("{}\n{} ", &caps[1], markdown::bold(&caps[2]))
                    })
            }
            RegexFilter::Bold => self.bold_re.replace_all(text, |caps: &Captures| -> String {
                markdown::bold(&caps[1])
            }),
            RegexFilter::Italic => self
                .italic_re
                .replace_all(text, |caps: &Captures| -> String {
                    markdown::italic(&caps[1])
                }),
        }
    }
}

impl PreppedMessage {
    pub fn build(item: &Item) -> PreppedMessage {
        let title = html2md::rewrite_html(item.title().unwrap_or(""), false);
        let mut content = None;
        let journal = Self::extract_journal(item);

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

    fn extract_journal(item: &Item) -> Option<String> {
        item.dublin_core_ext()?
            .clone()
            .sources()
            .iter()
            .next()
            .cloned()
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
                &Self::format_title(&self.title, ParseMode::MarkdownV2),
                "https://doi.org/",
                doi,
            ));
            result.push('\n');
            if let Some(journal) = &self.journal {
                result.push_str(&markdown::italic(&markdown::escape(journal)));
            }
            if let Some(content) = &self.content {
                result.push_str("\n\n");
                result.push_str(&PreppedMessage::format_abstract(
                    &markdown::escape(content),
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
            result.push('\n');
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
                result.push_str(&markdown::escape(content));
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

    fn format_markup(text: &str, parsemode: ParseMode) -> String {
        if parsemode == ParseMode::MarkdownV2 {
            // let mut text = markdown::escape(text);
            let mut text = text.to_string();
            text = text.replace(r"&lt;", r"\<");
            text = text.replace(r"&gt;", r"\>");
            text = text.replace(r"&amp;", r"&");

            let re = &*REGEXSTRUCT;

            text = re.apply(&text, RegexFilter::Bold).into_owned();
            re.apply(&text, RegexFilter::Italic).into_owned()
        } else {
            todo!()
        }
    }

    fn format_title(title: &str, parsemode: ParseMode) -> String {
        // Formats the abstract (escapes invalid characters, bolds RESULT: etc)
        let formatted = Self::format_markup(title, parsemode);
        formatted.replace(r"\_", r"")
    }

    fn format_abstract(content: &str, parsemode: ParseMode) -> String {
        // Formats the abstract (escapes invalid characters, bolds RESULT: etc)
        if parsemode == ParseMode::MarkdownV2 {
            let mut content = content.to_string();

            content = Self::format_markup(&content, parsemode);

            // Remove RSNA footer copyright
            if let Some(rsna_footer) = content.find(" ©RSNA") {
                content.truncate(rsna_footer)
            }

            let re = &*REGEXSTRUCT;
            // For AJR:
            content = re
                .apply(&content, RegexFilter::RemoveItalicKeyword)
                .into_owned();
            // For the journal "Radiology" and Acta radiologica (Sweden)
            content = re
                .apply(&content, RegexFilter::BoldKeyword)
                .trim()
                .to_string();

            re.apply(&content, RegexFilter::CapitalizeKeyword)
                .trim()
                .to_string()
        } else {
            todo!()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::channelwrapper::ChannelWrapper;
    use std::{fs::File, io::Read};

    use super::*;

    #[test]
    fn test_format() {
        let mut file = File::open("test/channel_abdominal_radiology.json").unwrap();
        let mut json = String::new();
        file.read_to_string(&mut json).unwrap();
        let channel = ChannelWrapper::from_json(&json).unwrap();

        let item = &channel.items[0];
        let message = PreppedMessage::build(item).format(ParseMode::MarkdownV2);
        let result = r"[Quantitative MRI radiomics approach for evaluating muscular alteration in Crohn disease: development of a machine learning\-nomogram composite diagnostic tool](https://doi\.org/10\.1007/s00261\-025\-04896\-x)
_Abdominal radiology \(New York\)_

*BACKGROUND:* Emerging evidence underscores smooth muscle hyperplasia and hypertrophy, rather than fibrosis, as the defining characteristics of fibrostenotic lesions in Crohn disease \(CD\)\. However, non\-invasive methods for quantifying these muscular changes have yet to be fully explored\.

*AIMS:* To explore the application value of radiomics based on magnetic resonance imaging \(MRI\) post\-contrast T1\-weighted images to identify muscular alteration in CD lesions with significant inflammation\.

*METHODS:* A total of 68 cases were randomly assigned in this study, with 48 cases allocated to the training dataset and the remaining 20 cases assigned to the independent test dataset\. Radiomic features were extracted and constructed a diagnosis model by univariate analysis and least absolute shrinkage and selection operator \(LASSO\) regression\. Construct a nomogram based on multivariate logistic regression analysis, integrating radiomics signature, MRI features and clinical characteristics\.

*RESULTS:* The radiomics model constructed based on the selected features of the post\-contrasted T1\-weighted images has good diagnostic performance, which yielded a sensitivity of 0\.880, a specificity of 0\.783, and an accuracy of 0\.833 \[AUC \= 0\.856, 95% confidence interval \(CI\) \= 0\.765\-0\.947\]\. Moreover, the nomogram representing the integrated model achieved good discrimination performances, which yielded a sensitivity of 0\.836, a specificity of 0\.892, and an accuracy of 0\.864 \(AUC \= 0\.926, 95% CI \= 0\.865\-0\.988\), and it was better than that of the radiomics model alone\.

*CONCLUSIONS:* The radiomics based on post\-contrasted T1\-weighted images provides additional biomarkers for Crohn disease\. Additionally, integrating DCE\-MRI, radiomics, and clinical data into a comprehensive model significantly improves diagnostic accuracy for identifying muscular alteration\.
[Link](https://doi\.org/10\.1007/s00261\-025\-04896\-x) \| [PubMed](https://pubmed\.ncbi\.nlm\.nih\.gov/40232416) \| [QxMD](https://qxmd\.com/r/40232416)";

        assert_eq!(message, result);

        let mut file = File::open("test/channel_radiology.json").unwrap();
        let mut json = String::new();
        file.read_to_string(&mut json).unwrap();
        let channel = ChannelWrapper::from_json(&json).unwrap();

        let item = &channel.items[7];
        let message = PreppedMessage::build(item).format(ParseMode::MarkdownV2);
        let result = r"[Intraprotocol Adrenal Vein Sampling Inconsistencies in Primary Aldosteronism Lateralization](https://doi\.org/10\.1148/radiol\.240631)
_Radiology_

*BACKGROUND:* Primary aldosteronism can arise from one or both adrenal glands\. Adrenal vein sampling \(AVS\) is the standard of care for identifying patients with lateralized primary aldosteronism who would benefit from surgery\. Variability in AVS lateralization has been primarily attributed to cosyntropin use and lateralization index thresholds\. Data regarding intraprotocol variability are lacking\.

*PURPOSE:* To assess the rates of intraprotocol lateralization inconsistency during simultaneous AVS\.

*MATERIALS AND METHODS:* This retrospective cross\-sectional study assessed patients with primary aldosteronism who underwent simultaneous AVS at a single tertiary referral center between January 2015 and December 2023\. Six sets of adrenal vein and peripheral vein samples were obtained: three baseline samples obtained after cannulation, 5 minutes apart; and three samples obtained between 5 and 30 minutes after cosyntropin stimulation\. Patients with successful cannulation and valid hormonal data at all six time points were included\. A lateralization index \(computed as the aldosterone\-to\-cortisol ratio between the two adrenal veins, with the highest number as numerator\) of at least 4 was considered indicative of lateralized primary aldosteronism\. The proportions of baseline and stimulated AVS sets within which one of three lateralization indexes provided different subtype results were assessed\. Linear mixed\-effects models were used to estimate the between\- and within\-patient hormonal and lateralization index variances\.

*RESULTS:* Of 402 patients \(median age, 53 years; IQR, 45\-63 years; 233 male\) included, 129 patients \(32\.1%\) had at least one lateralization index inconsistency\. Of these 402 patients, 89 patients \(22\.1%\) had lateralization inconsistencies within the baseline sets, 53 patients \(13\.2%\) within cosyntropin\-stimulated sets, and 13 patients \(3\.2%\) in both baseline and cosyntropin\-stimulated sets\. The highest outlier prevalence occurred in the first \(42 patients; 10\.4%\) and third \(33 patients; 8\.2%\) baseline samples, with roughly twofold\-lower rates in the first \(23 patients; 5\.7%\) and last postcosyntropin stimulation samples \(4\.2%; 17 patients\)\. The absolute change in baseline and cosyntropin\-stimulated lateralization index \(maximum\-minimum lateralization index within a triplicate\) was as high as 152\.9 and 327\.4, respectively\. The highest hormonal variability was noted in the adrenal vein producing less aldosterone\.

*CONCLUSION:* Almost a third of patients undergoing AVS in triplicate, both before and after cosyntropin stimulation, had intraprotocol discrepancies in lateralization results, with the highest variability occurring within samples obtained without cosyntropin stimulation\.
[Link](https://doi\.org/10\.1148/radiol\.240631) \| [PubMed](https://pubmed\.ncbi\.nlm\.nih\.gov/40232138) \| [QxMD](https://qxmd\.com/r/40232138)";

        assert_eq!(message, result);

        let mut file = File::open("test/channel_AJR.json").unwrap();
        let mut json = String::new();
        file.read_to_string(&mut json).unwrap();
        let channel = ChannelWrapper::from_json(&json).unwrap();

        let item = &channel.items[0];
        let message = PreppedMessage::build(item).format(ParseMode::MarkdownV2);
        let result = r"[Interreader Agreement of Lung\-RADS: A Systematic Review and Meta\-Analysis](https://doi\.org/10\.2214/AJR\.25\.32681)
_AJR\. American journal of roentgenology_

*BACKGROUND:* Lung\-RADS has shown variable interreader agreement in the literature, in part related to a broad range of factors that may influence the consistency of its implementation\.

*OBJECTIVE:* To assess the interreader agreement of Lung\-RADS and to investigate factors influencing the system's variability\.

*EVIDENCE ACQUISITION:* EMBASE, PubMed, and Cochrane databases were searched for original research studies published through June 18, 2024 reporting the interreader agreement of Lung\-RADS on chest CT\. Random\-effect models were used to calculate pooled kappa coefficients for Lung\-RADS categorization and pooled intraclass correlation coefficients \(ICCs\) for nodule size measurements\. Potential sources of heterogeneity were explored using metaregression analyses\.

*EVIDENCE SYNTHESIS:* The analysis included 11 studies \(1470 patients\) for Lung\-RADS categorization and five studies \(617 patients\) for nodule size measurement\. Interreader agreement for Lung\-RADS categorization was substantial \(κ\=0\.72 \[95% CI, 0\.57\-0\.82\]\), and for nodule size measurement was almost perfect \(ICC\=0\.97 \[95% CI, 0\.90\-0\.99\]\)\. Interreader agreement for Lung\-RADS categorization was significantly associated with the method of nodule measurement \(p\=\.005\), with pooled kappa coefficients for studies using computer\-aided detection \(CAD\)\-based semiautomated volume measurements, using CAD\-based semiautomated diameter measurements, and using manual diameter measurements of 0\.95, 0\.91, and 0\.66, respectively\. Interreader agreement for Lung\-RADS categorization was also significantly associated with studies' nodule type distribution \(p\<\.001\), with pooled kappa coefficients for studies evaluating 100% solid nodules, 30\-99% solid nodules, and \<30% solid nodules of 0\.85, 0\.76, and 0\.55, respectively\. Interreader agreement fornodule size measurement was significantly associated with radiation dose \(p\<\.001\), with pooled ICCs for studies that used standard\-dose CT, used low\-dose CT, and used ultralow\-dose CT of 0\.97, 0\.96, and 0\.59, respectively\. Interreader agreement for nodule size measurement was also significantly associated with the Lung\-RADS version used \(p\=\.02\), with pooled ICCs for studies using Lung\-RADS 1\.1 and using Lung\-RADS 1\.0 of 0\.99 and 0\.93, respectively\.

*CONCLUSION:* While supporting the overall reliability of Lung\-RADS, the findings indicate roles for CAD assistance as well as training and standardized approaches for nodule type characterization to further promote reproducible application\.

*CLINICAL IMPACT:* Consistent nodule assessments will be critical for Lung\-RADS to optimally impact patient management and outcomes\.
[Link](https://doi\.org/10\.2214/AJR\.25\.32681) \| [PubMed](https://pubmed\.ncbi\.nlm\.nih\.gov/40202356) \| [QxMD](https://qxmd\.com/r/40202356)";
        assert_eq!(message, result);
    }
}
