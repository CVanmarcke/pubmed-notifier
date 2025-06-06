use std::collections::HashSet;
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, EnumString};
// use strum_macros::EnumIter;

pub enum PresetList {
    Keyword(HashSet<String>),
    Journal(HashSet<u32>),
}

pub enum Preset {
    Keyword(Keywords),
    Journal(Journals),
}

#[derive(Debug, EnumIter, EnumString)]
pub enum Keywords {
    #[strum(ascii_case_insensitive)]
    Uro,
    #[strum(ascii_case_insensitive)]
    Abdomen,
    #[strum(ascii_case_insensitive)]
    DefaultBlacklist,
    #[strum(ascii_case_insensitive)]
    AIBlacklist,
}

#[derive(Debug, EnumIter, EnumString)]
pub enum Journals {
    #[strum(ascii_case_insensitive)]
    Radiology,
    #[strum(ascii_case_insensitive)]
    TechnicalRadiology,
    #[strum(ascii_case_insensitive)]
    Clinical,
    #[strum(ascii_case_insensitive)]
    ClinicalUrology,
    #[strum(ascii_case_insensitive)]
    ClinicalGI,
}

const DEFAULT_URO_WHITELIST: &[&str] = &[
    "urogenital",
    "genitourina",
    "urinary",
    "renal",
    "kidney",
    " bladder",
    "vesical",
    "urothelial",
    "prostat",
    "seminal",
    "penis",
    "testic",
    "scrotum",
    "scrotal",
];

const DEFAULT_BLACKLIST: &[&str] = &[
    "letter to the editor",
    "erratum for: ",
    "editorial comment",
    "response to \"",
];

const AI_BLACKLIST: &[&str] = &[
    "radiomic",
    "nomogram",
    "deep learning",
    "deep-learning",
    "artificial intelligence",
    "histogram",
];

const DEFAULT_ABDOMEN_WHITELIST: &[&str] = &[
    "abdomen",
    "abdominal",
    "peritoneum",
    "peritoneal",
    "perineal",
    "perineum",
    " liver",
    "hepatic",
    "hepato",
    "HCC",
    "biliar",
    "gallbladder",
    "pancrea",
    "spleen",
    "splenic",
    "gastro",
    "gastric",
    "duoden",
    "jejun",
    "ileum",
    "ileal",
    "colon",
    "sigmoid",
    "rectum",
    "rectal",
    " anus",
    " anal ",
    "uterus",
    "uterine",
    "endometri",
    "ovary",
    "ovarian",
    " omentum",
    "omental",
    "adnex",
    "cervix",
    "vagina",
    "cervical ca",
];

const DEFAULT_RADIOLOGY_JOURNALS: &[u32] = &[
    101532453, 101674571, 8302501, 7708173, 9114774, 401260, 101765309, 8106411, 100956096,
    101490689, 8911831, 1306016, 101698198, 8706123, 101721752,
];

const TECHNICAL_RADIOLOGY_JOURNALS: &[u32] = &[
    8505245, 9105850, 9707935, 7703942, 9440159, 8211547, 101626019, 101315005,
];
const CLINICAL_UROLOGY_JOURNALS: &[u32] = &[7512719, 376374, 101724904];
const CLINICAL_GI_JOURNALS: &[u32] = &[374630, 100966936];
const CLINICAL_JOURNALS: &[u32] = &[100909747, 101589553, 255562];

pub fn available_presets() -> String {
    let mut s = String::from("Keyword presets:\n");
    for keyword in Keywords::iter() {
        s.push_str(&format!("- {:?}\n", keyword));
    }
    s.push_str("- Journal presets:\n");
    for journal in Journals::iter() {
        s.push_str(&format!("- {:?}\n", journal));
    }
    s
}

pub fn get_preset_keywords(keywords: Keywords) -> HashSet<String> {
    match keywords {
        Keywords::Uro => DEFAULT_URO_WHITELIST,
        Keywords::Abdomen => DEFAULT_ABDOMEN_WHITELIST,
        Keywords::DefaultBlacklist => DEFAULT_BLACKLIST,
        Keywords::AIBlacklist => AI_BLACKLIST,
    }
    .iter()
    .map(|x| x.to_string())
    .collect::<HashSet<String>>()
}

pub fn get_preset(preset: Preset) -> PresetList {
    match preset {
        Preset::Journal(j) => PresetList::Journal(get_preset_journals(j)),
        Preset::Keyword(k) => PresetList::Keyword(get_preset_keywords(k)),
    }
}

pub fn get_preset_journals(journals: Journals) -> HashSet<u32> {
    match journals {
        Journals::Radiology => DEFAULT_RADIOLOGY_JOURNALS,
        Journals::Clinical => CLINICAL_JOURNALS,
        Journals::TechnicalRadiology => TECHNICAL_RADIOLOGY_JOURNALS,
        Journals::ClinicalUrology => CLINICAL_UROLOGY_JOURNALS,
        Journals::ClinicalGI => CLINICAL_GI_JOURNALS,
    }
    .iter()
    .copied()
    .collect::<HashSet<u32>>()
}

pub fn merge_keyword_preset_with_set(keywords: Keywords, set: &HashSet<String>) -> HashSet<String> {
    get_preset_keywords(keywords)
        .into_iter()
        .chain(set.iter().cloned())
        .collect::<HashSet<String>>()
}
pub fn merge_journal_preset_with_set(journals: Journals, set: &HashSet<u32>) -> HashSet<u32> {
    get_preset_journals(journals)
        .into_iter()
        .chain(set.iter().cloned())
        .collect::<HashSet<u32>>()
}

pub fn parse_preset(preset_str: &str) -> Option<Preset> {
    if let Ok(p) = Journals::from_str(preset_str) {
        Some(Preset::Journal(p))
    } else if let Ok(p) = Keywords::from_str(preset_str) {
        Some(Preset::Keyword(p))
    } else {
        None
    }
}
