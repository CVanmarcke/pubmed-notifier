use std::collections::HashSet;

pub enum Preset {
    Keyword(Keywords),
    Journal(Journals)
}

pub enum Keywords {
    Uro,
    Abdomen,
    DefaultBlacklist,
    AIBlacklist
}

pub enum Journals {
    Radiology
}

pub fn available_presets() -> [&'static str; 5] {
    ["uro", "abdomen", "default_blacklist", "ai_blacklist", "radiology_journals"]
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
    "Letter to the Editor",
    "Erratum for: ",
    "Editorial Comment",
];

const AI_BLACKLIST: &[&str] = &[
    "radiomic",
    "nomogram",
    "deep learning",
    "deep-learning",
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
    "anus",
    " anal ",
    "uterus",
    "uterine",
    "ovary",
    "ovarian",
    "omentum",
    "omental",
    "adnex",
    "cervix",
    "vagina",
    "cervical ca",
];

const DEFAULT_RADIOLOGY_JOURNALS: [u32; 15] = [
    101532453, 101674571, 8302501, 7708173, 9114774, 0401260, 101765309, 8106411, 100956096,
    101490689, 8911831, 1306016, 101698198, 8706123, 101721752,
];


pub fn radiology_journals() -> HashSet<u32> {
    HashSet::from(DEFAULT_RADIOLOGY_JOURNALS)
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

// pub fn get_preset<T>(preset: Preset) -> HashSet<T> {
//     match preset {
//         Preset::Journal( j ) => get_preset_journals(j),
//         Preset::Keyword( k ) => get_preset_keywords(k),
//     }
// }

pub fn get_preset_journals(journals: Journals) -> HashSet<u32> {
    HashSet::from(
        match journals {
            Journals::Radiology => DEFAULT_RADIOLOGY_JOURNALS,
        })
}

pub fn merge_preset_with_set(keywords: Keywords, set: &HashSet<String>)  -> HashSet<String> {
    get_preset_keywords(keywords)
        .into_iter()
        .chain(set.iter().cloned())
        .collect::<HashSet<String>>()
}
