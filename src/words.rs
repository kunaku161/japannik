use rusqlite::params;
use std::collections::{HashMap, HashSet};
use crate::config::FrequencyBias;
use crate::db;

pub struct WordEntry {
    pub lemma: String,
    pub word_kana: String,
    /// Translations keyed by JMdict language code; multiple meanings joined with ';' at output time.
    pub translations: HashMap<String, Vec<String>>,
    /// Populated when config.output.output_freq_score = true.
    pub freq_score: Option<f64>,
}

/// Looks up each lemma in the DB and returns kana + translations for the requested languages.
/// Words not found in JMdict are returned with empty translation and kana fields.
pub fn lookup_words(db_path: &str, lemmas: &[&str], languages: &[String]) -> Vec<WordEntry> {
    let conn = db::open(db_path);

    lemmas
        .iter()
        .map(|&lemma| {
            let word_info: Option<(i64, String)> = conn
                .query_row(
                    "SELECT id, word_kana FROM words WHERE word = ?1 OR word_kana = ?1 LIMIT 1",
                    params![lemma],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .ok();

            let Some((word_id, word_kana)) = word_info else {
                return WordEntry {
                    lemma: lemma.to_string(),
                    word_kana: String::new(),
                    translations: HashMap::new(),
                    freq_score: None,
                };
            };

            let mut stmt = conn
                .prepare_cached("SELECT language, meaning FROM glosses WHERE word_id = ?1")
                .expect("failed to prepare gloss query");

            let mut translations: HashMap<String, Vec<String>> = HashMap::new();

            stmt.query_map(params![word_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("gloss query failed")
            .filter_map(|r| r.ok())
            .filter(|(lang, _)| languages.contains(lang))
            .for_each(|(lang, meaning)| {
                translations.entry(lang).or_default().push(meaning);
            });

            WordEntry {
                lemma: lemma.to_string(),
                word_kana,
                translations,
                freq_score: None,
            }
        })
        .collect()
}

/// Returns (lemma, weighted_score) for each lemma, scored by freq_total + bias weights.
/// Words not found in the DB get score 0.0.
pub fn score_lemmas(db_path: &str, lemmas: &[&str], bias: &FrequencyBias) -> Vec<(String, f64)> {
    let conn = db::open(db_path);

    lemmas
        .iter()
        .map(|&lemma| {
            let score: f64 = conn
                .query_row(
                    "SELECT freq_total,
                            freq_howto, freq_science, freq_entertainment, freq_education,
                            freq_people, freq_music, freq_autos, freq_comedy, freq_film,
                            freq_gaming, freq_sports, freq_news, freq_nonprofits,
                            freq_travel, freq_pets
                     FROM words WHERE word = ?1 OR word_kana = ?1 LIMIT 1",
                    params![lemma],
                    |row| {
                        Ok(bias.score(
                            row.get(0)?,
                            row.get(1)?,  row.get(2)?,  row.get(3)?,  row.get(4)?,
                            row.get(5)?,  row.get(6)?,  row.get(7)?,  row.get(8)?,
                            row.get(9)?,  row.get(10)?, row.get(11)?, row.get(12)?,
                            row.get(13)?, row.get(14)?, row.get(15)?,
                        ))
                    },
                )
                .unwrap_or(0.0);
            (lemma.to_string(), score)
        })
        .collect()
}

/// Returns all words marked is_known = 1 as a flat HashSet of both their kanji
/// and kana forms so callers can do a single .contains() check against any word form.
pub fn load_known_words(db_path: &str) -> HashSet<String> {
    let conn = db::open(db_path);
    let mut stmt = conn
        .prepare("SELECT word, word_kana FROM words WHERE is_known = 1")
        .expect("failed to prepare known words query");

    let mut known = HashSet::new();
    stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .expect("failed to query known words")
        .filter_map(|r| r.ok())
        .for_each(|(word, word_kana)| {
            known.insert(word);
            known.insert(word_kana);
        });

    known
}

// Called by CLI "add known words" action.
#[allow(dead_code)]
/// Marks words as known without resetting existing flags.
/// Use this for the "Add known words" action — knowledge only grows, never shrinks automatically.
// TODO/TOTEST: What if only kana and too many words get updated?
pub fn add_known_words(db_path: &str, words: &HashSet<String>) {
    let conn = db::open(db_path);
    let mut matched = 0usize;
    for word in words {
        let n = conn
            .execute(
                "UPDATE words SET is_known = 1 WHERE word = ?1 OR word_kana = ?1",
                params![word],
            )
            .expect("failed to update known word");
        if n == 0 {
            println!("[debug] known word not in JMdict: '{}'", word);
        }
        matched += n;
    }
    println!(
        "Known words: {} entries added, {} DB rows marked",
        words.len(),
        matched
    );
}

// Called by CLI "reset known words" action.
#[allow(dead_code)]
/// Clears all is_known flags. Only called by an explicit user action, never on startup.
pub fn reset_known_words(db_path: &str) {
    let conn = db::open(db_path);
    conn.execute_batch("UPDATE words SET is_known = 0")
        .expect("failed to reset known words");
    println!("All known words reset.");
}
