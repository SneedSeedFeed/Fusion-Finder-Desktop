use std::{
    collections::HashMap,
    num::ParseIntError,
    path::{Path, PathBuf},
};

use regex::Regex;
use snafu::{OptionExt, ResultExt, Snafu};

pub struct NameMap {
    map: Box<[NameHalves]>,
    nat_dex_mapping: HashMap<u16, usize>,
}

#[derive(Debug, Snafu)]
pub enum FromFileError {
    #[snafu(display("Failed to read file {}", path.display()))]
    ReadFile {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("Failed to parse file contents"))]
    Parse { source: FromContentError },
}

#[derive(Debug, Snafu)]
pub enum FromContentError {
    #[snafu(display("Failed to parse name halves"))]
    ParseNameHalves { source: ParseNameHalvesError },

    #[snafu(display("Failed to parse nat dex mapping"))]
    ParseNatDexMapping { source: ParseNatDexMappingError },
}

impl NameMap {
    fn get_idx(&self, fusion_dex_num: u16) -> Option<usize> {
        if fusion_dex_num < 252 {
            Some(fusion_dex_num as usize)
        } else {
            self.nat_dex_mapping.get(&fusion_dex_num).copied()
        }
    }

    pub fn get_name_halves(&self, fusion_dex_num: u16) -> Option<&NameHalves> {
        self.get_idx(fusion_dex_num)
            .and_then(|idx| self.map.get(idx))
    }

    pub fn from_file<P: AsRef<Path>>(file_path: P) -> Result<Self, FromFileError> {
        let path = file_path.as_ref();
        let content = std::fs::read_to_string(path).context(ReadFileSnafu { path })?;
        Self::from_content(&content).context(ParseSnafu)
    }

    pub fn from_content(content: &str) -> Result<Self, FromContentError> {
        let map = parse_name_halves(content).context(ParseNameHalvesSnafu)?;
        let nat_dex_mapping = parse_nat_dex_mapping(content).context(ParseNatDexMappingSnafu)?;

        Ok(NameMap {
            map: map.into_boxed_slice(),
            nat_dex_mapping,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NameHalves {
    pub first_half: Box<str>,
    pub second_half: Box<str>,
}

#[derive(Debug, Snafu)]
pub enum ParseNameHalvesError {
    #[snafu(display("Failed to compile the SPLIT_NAMES array regex"))]
    BuildArrayRegex { source: regex::Error },

    #[snafu(display("Could not find the SPLIT_NAMES array in the file"))]
    ArrayNotFound,

    #[snafu(display("Failed to compile the name-halves element regex"))]
    BuildElementsRegex { source: regex::Error },
}

fn parse_name_halves(rb_file: &str) -> Result<Vec<NameHalves>, ParseNameHalvesError> {
    const SPLIT_NAMES_REGEX: &str = r"SPLIT_NAMES\s*=\s*(\[(?:[^\[\]]|\[[^\[\]]*\])*\])";
    const ELEMENTS_REGEX: &str = r#"\["([^"]*)",\s*"([^"]*)"\]"#;

    let array_regex = Regex::new(SPLIT_NAMES_REGEX).context(BuildArrayRegexSnafu)?;
    let array_match = array_regex
        .find(rb_file)
        .context(ArrayNotFoundSnafu)?
        .as_str();

    let halves_regex = Regex::new(ELEMENTS_REGEX).context(BuildElementsRegexSnafu)?;
    let mut halves = Vec::<NameHalves>::new();
    for elem in halves_regex.captures_iter(array_match) {
        let first = &elem[1];
        let second = &elem[2];
        halves.push(NameHalves {
            first_half: first.into(),
            second_half: second.into(),
        });
    }
    halves.shrink_to_fit();
    Ok(halves)
}

#[derive(Debug, Snafu)]
pub enum ParseNatDexMappingError {
    #[snafu(display("Failed to compile the NAT_DEX_MAPPING regex"))]
    BuildMappingRegex { source: regex::Error },

    #[snafu(display("Could not find the NAT_DEX_MAPPING hash in the file"))]
    MappingNotFound,

    #[snafu(display("Failed to compile the key/value pairs regex"))]
    BuildPairsRegex { source: regex::Error },

    #[snafu(display("Failed to parse {:?} as a dex-number key", value))]
    ParseKey {
        source: ParseIntError,
        value: String,
    },

    #[snafu(display("Failed to parse {:?} as a dex-number value", value))]
    ParseValue {
        source: ParseIntError,
        value: String,
    },
}

fn parse_nat_dex_mapping(rb_file: &str) -> Result<HashMap<u16, usize>, ParseNatDexMappingError> {
    const NAT_DEX_REGEX: &str = r"NAT_DEX_MAPPING\s*=\s*(\{[^}]*\})";
    let nat_dex_re = Regex::new(NAT_DEX_REGEX).context(BuildMappingRegexSnafu)?;

    let hash_match = nat_dex_re
        .find(rb_file)
        .context(MappingNotFoundSnafu)?
        .as_str();

    const PAIRS_REGEX: &str = r"(\d+)\s*=>\s*(\d+)";
    let pairs_re = Regex::new(PAIRS_REGEX).context(BuildPairsRegexSnafu)?;

    let mut pairs = HashMap::new();

    for elem in pairs_re.captures_iter(hash_match) {
        let key = elem[1].parse().context(ParseKeySnafu { value: &elem[1] })?;
        let value = elem[2]
            .parse()
            .context(ParseValueSnafu { value: &elem[2] })?;
        pairs.insert(key, value);
    }
    pairs.shrink_to_fit();
    Ok(pairs)
}

#[cfg(test)]
mod test {
    use crate::test::infinite_fusion_dir;

    use super::{parse_name_halves, parse_nat_dex_mapping};

    #[test]
    fn parse_halves_test() {
        let data = std::fs::read_to_string(
            infinite_fusion_dir().join("Data\\Scripts\\052_InfiniteFusion\\Fusion\\SplitNames.rb"),
        )
        .unwrap();
        let name_halves = parse_name_halves(&data).unwrap();

        let nat_dex_mapping = parse_nat_dex_mapping(&data).unwrap();

        assert!(!name_halves.is_empty());
        assert!(!nat_dex_mapping.is_empty());
    }
}
