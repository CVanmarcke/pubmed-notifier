use std::collections::HashSet;

// TODO:
// does not pass blacklist https://pubs.rsna.org/doi/10.1148/rg.240151
// No whitelist O-RADS US Version 2022 Improves Patient Risk Stratification When Compared with O-RADS US Version 2019
// no whitelist: Photon-counting CT: The New Kid on the Block for Liver Fat Quantification
// item is in the iter list: A New Challenge in Prostate Cancer: Assessing Discrepant Results from Prostate MRI and PSMA PET/CT


// item passed whitelist: Deep Learning Radiopathomics Models Based on Contrast-enhanced MRI and Pathologic Imaging for Predicting Vessels Encapsulating Tumor Clusters and Prognosis in Hepatocellular Carcinoma

// item is in the iter list: Prostate Cancer Screening: Empirical Clinical Practice for 70 Years

// item is in the iter list: Enhancing Practices for Multiparametric MRI in Gastric Cancer: Addressing Clear Criteria for T and N Stage

// item passed whitelist: Preoperative MRI-based predictive model for biochemical recurrence following radical prostatectomy
// .........


pub fn available_presets() -> [&'static str; 4] {
    ["uro", "abdomen", "default_blacklist", "radiology_journals"]
}

const DEFAULT_URO_WHITELIST: &[&str] = &["urogenital", "genitourina", "urinary",
    "renal", "kidney", " bladder", "vesical", "urothelial",
    "prostat", "seminal", "penis", "testic", "scrotum", "scrotal"];
    
const DEFAULT_BLACKLIST: &[&str] = &["Letter to the Editor", "Erratum for: ", "Editorial Comment",
    "radiomic", "nomogram", "deep learning", "deep-learning","histogram"];

const DEFAULT_ABDOMEN_WHITELIST: &[&str] = &["abdomen", "abdominal", "peritoneum", "peritoneal", "perineal", "perineum",
    " liver", "hepatic", "hepato", "HCC", "biliar", "gallbladder",
    "pancrea", "spleen", "splenic",
    "gastro", "gastric", "duoden", "jejun", "ileum", "ileal",
    "colon", "sigmoid", "rectum", "rectal", "anus", " anal ",
    "uterus", "uterine", "ovary", "ovarian", "adnex", "cervix", "vagina", "cervical ca"];

const DEFAULT_RADIOLOGY_JOURNALS: [u32; 15] = [101532453, 101674571, 8302501, 7708173, 9114774, 0401260, 101765309, 8106411, 100956096, 101490689, 8911831, 1306016, 101698198, 8706123, 101721752];

pub fn uro_whitelist() -> HashSet<String> {
    DEFAULT_URO_WHITELIST
        .iter()
        .map(|x| x.to_string())
        .collect::<HashSet<String>>()
}

pub fn abdomen_whitelist() -> HashSet<String> {
    DEFAULT_ABDOMEN_WHITELIST
        .iter()
        .map(|x| x.to_string())
        .collect::<HashSet<String>>()
}

pub fn default_blacklist() -> HashSet<String> {
    DEFAULT_BLACKLIST
        .iter()
        .map(|x| x.to_string())
        .collect::<HashSet<String>>()
}

pub fn radiology_journals() -> HashSet<u32> {
    HashSet::from(DEFAULT_RADIOLOGY_JOURNALS)
}


// #[derive(Debug)]
// pub enum PresetData {
//     Keyword(HashSet<String>),
//     Journal(HashSet<u32>),
// }

// pub enum PresetKind {
//     Keyword,
//     Journal,
// }
// #[derive(Debug)]
// pub enum Preset {
//     RadiologyJournal,
//     UroKeywords,
//     AbdomenKeywords,
//     DefaultBlacklist
// }

// impl PresetData {
//     pub fn get_data(&self) {
//         match self {
//             PresetData::Keyword(set) => set,
//             PresetData::Journal(set) => set,
//         }
//     }
// }

// impl Preset {
//     pub fn parse(s: String) -> Option<Preset> {
//         match s.to_lowercase() {
//             x if x.contains("radiology") => Some(Preset::RadiologyJournal),
//             x if x.contains("urokeywords") => Some(Preset::UroKeywords),
//             x if x.contains("abdomenkeywords") => Some(Preset::AbdomenKeywords),
//             x if x.contains("defaultblacklist") => Some(Preset::DefaultBlacklist),
//             _ => None
//         }
//     }
//     pub fn get_data(&self) -> PresetData {
//         match self {
//             Preset::RadiologyJournal => PresetData::Journal(HashSet::from([1])),
//             Preset::UroKeywords => PresetData::Keyword(HashSet::from(
//                 DEFAULT_URO_WHITELIST
//                     .iter()
//                     .map(|x| x.to_string())
//                     .collect::<HashSet<String>>())),
//             Preset::AbdomenKeywords => PresetData::Keyword(HashSet::from(
//                 DEFAULT_ABDOMEN_WHITELIST
//                     .iter()
//                     .map(|x| x.to_string())
//                     .collect::<HashSet<String>>())),
//             Preset::DefaultBlacklist => PresetData::Keyword(HashSet::from(
//                 DEFAULT_BLACKLIST
    
//                     .map(|x| x.to_string())
//                     .collect::<HashSet<String>>())),
//         }
        
//     }
// }

