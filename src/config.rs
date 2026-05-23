use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub paths: PathsConfig,
    pub general: GeneralConfig,
    pub output: OutputConfig,
    pub frequency_bias: FrequencyBias,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            paths: PathsConfig::default(),
            general: GeneralConfig::default(),
            output: OutputConfig::default(),
            frequency_bias: FrequencyBias::default(),
        }
    }
}

impl Config {
    /// Loads from `path` if it exists; falls back to defaults on missing file or parse error.
    pub fn load_or_default(path: &str) -> Self {
        let content = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return Config::default(),
        };
        match toml::from_str(&content) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Warning: failed to parse {path}: {e}\nUsing default configuration.");
                Config::default()
            }
        }
    }
}

// ── Paths ────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    pub db: String,
    pub jmdict: String,
    pub frequency: String,
    pub known_words_dir: String,
    pub sudachi_dict: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        PathsConfig {
            db: "japannik.db".to_string(),
            jmdict: "resources/JMdict".to_string(),
            frequency: "resources/frequency_spoken.tsv".to_string(),
            known_words_dir: "known_words".to_string(),
            sudachi_dict: "resources/system.dic".to_string(),
        }
    }
}

// ── General ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub sudachi_mode: SudachiMode,
    pub ignore_known_words: bool,
    pub normalize_known_words: bool,
    pub verbose: bool,
    /// Column index (0-based) in Anki TSV exports that holds the clean word.
    pub known_words_col: usize,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        GeneralConfig {
            sudachi_mode: SudachiMode::Idioms,
            ignore_known_words: true,
            normalize_known_words: true,
            verbose: false,
            known_words_col: 0,
        }
    }
}

/// Sudachi segmentation granularity.
/// - Single:   smallest morpheme units (助動詞, ない split from verb)
/// - Compound: standard dictionary words (日本語 as one token)
/// - Idioms:   largest units including set phrases (気がする as one token)
#[derive(Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SudachiMode {
    Single,
    Compound,
    #[default]
    Idioms,
}

// ── Output ───────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct OutputConfig {
    /// Top N unknown words from the input text, ranked by how often they appear there.
    pub max_from_text: usize,
    /// Top M words from the remaining text unknowns, ranked by general corpus frequency.
    /// Catches high-value words that happened to appear rarely in this specific text.
    pub max_from_corpus: usize,
    /// Reserved for a future sentence-mining feature. Leave at 0 for now.
    pub max_from_sentences: usize,
    /// For the sentence feature: a "perfect" sentence has at most this many unknown words.
    pub max_unknown_in_sentence: usize,
    /// Ignore words appearing fewer than this many times in the input text.
    pub min_text_occurrences: usize,
    /// JMdict language codes to include as translation columns in the CSV.
    pub output_languages: Vec<String>,
    /// Include the computed corpus frequency score as a column in the vocab CSV.
    pub output_freq_score: bool,
}

impl Default for OutputConfig {
    fn default() -> Self {
        OutputConfig {
            max_from_text: 10,
            max_from_corpus: 5,
            max_from_sentences: 0,
            max_unknown_in_sentence: 2,
            min_text_occurrences: 1,
            output_languages: vec!["de".to_string(), "en".to_string()],
            output_freq_score: false,
        }
    }
}

// ── Frequency bias ───────────────────────────────────────────────────────────

/// Per-category multipliers added to freq_total when scoring words for Pool B.
/// score = freq_total + gaming * freq_gaming + film * freq_film + …
/// All weights default to 0.0 (rank by total frequency only).
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct FrequencyBias {
    pub howto: f64,
    pub science: f64,
    pub entertainment: f64,
    pub education: f64,
    pub people: f64,
    pub music: f64,
    pub autos: f64,
    pub comedy: f64,
    pub film: f64,
    pub gaming: f64,
    pub sports: f64,
    pub news: f64,
    pub nonprofits: f64,
    pub travel: f64,
    pub pets: f64,
}

impl FrequencyBias {
    /// Computes the weighted score for a word given its raw category counts.
    pub fn score(
        &self,
        freq_total: i64,
        freq_howto: i64, freq_science: i64, freq_entertainment: i64, freq_education: i64,
        freq_people: i64, freq_music: i64, freq_autos: i64, freq_comedy: i64,
        freq_film: i64, freq_gaming: i64, freq_sports: i64, freq_news: i64,
        freq_nonprofits: i64, freq_travel: i64, freq_pets: i64,
    ) -> f64 {
        freq_total as f64
            + self.howto         * freq_howto         as f64
            + self.science       * freq_science        as f64
            + self.entertainment * freq_entertainment  as f64
            + self.education     * freq_education      as f64
            + self.people        * freq_people         as f64
            + self.music         * freq_music          as f64
            + self.autos         * freq_autos          as f64
            + self.comedy        * freq_comedy         as f64
            + self.film          * freq_film           as f64
            + self.gaming        * freq_gaming         as f64
            + self.sports        * freq_sports         as f64
            + self.news          * freq_news           as f64
            + self.nonprofits    * freq_nonprofits     as f64
            + self.travel        * freq_travel         as f64
            + self.pets          * freq_pets           as f64
    }
}

impl Default for FrequencyBias {
    fn default() -> Self {
        FrequencyBias {
            howto: 0.0, science: 0.0, entertainment: 0.0, education: 0.0,
            people: 0.0, music: 0.0, autos: 0.0, comedy: 0.0, film: 0.0,
            gaming: 0.0, sports: 0.0, news: 0.0, nonprofits: 0.0,
            travel: 0.0, pets: 0.0,
        }
    }
}
