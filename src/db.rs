use quick_xml::events::Event;
use quick_xml::Reader;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::fs;

// TODO: refactor into 2 + n files:
// 1. DB Initialisation. For alters, updates, Init
// 2. DB Operations like get all known_words or shit like this with multiple rows
// n. for every table one Object with fields etc. Created by ID (Also for List of words (as Enum eg.)

pub fn init_dictionary(db_path: &str, jmdict_path: &str) {
    let conn = Connection::open(db_path).expect("failed to open DB");

    conn.execute_batch("
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        CREATE TABLE IF NOT EXISTS words (
            id        INTEGER PRIMARY KEY,
            word      TEXT NOT NULL,
            word_kana TEXT NOT NULL,
            pos       TEXT,
            is_known  BOOLEAN NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS glosses (
            id      INTEGER PRIMARY KEY AUTOINCREMENT,
            language TEXT NOT NULL,
            meaning  TEXT NOT NULL,
            word_id  INTEGER NOT NULL,
            FOREIGN KEY (word_id) REFERENCES words(id)
        );
    ").expect("failed to create tables");

    // Migrate existing DBs that predate the is_known column — silently ignored if already present
    let _ = conn.execute_batch(
        "ALTER TABLE words ADD COLUMN is_known BOOLEAN NOT NULL DEFAULT 0",
    );

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM words", [], |row| row.get(0))
        .unwrap_or(0);
    if count > 0 {
        println!("Dictionary already initialized ({} words), skipping import.", count);
    } else {
        parse_and_insert(&conn, jmdict_path);
    }

}

// Category frequency columns — order matches the TSV (cols 1, 4–18).
// Exposed so callers can build weighted scores with user-configured bias.
pub const FREQ_COLS: &[&str] = &[
    "freq_total",
    "freq_howto", "freq_science", "freq_entertainment", "freq_education",
    "freq_people", "freq_music", "freq_autos", "freq_comedy", "freq_film",
    "freq_gaming", "freq_sports", "freq_news", "freq_nonprofits",
    "freq_travel", "freq_pets",
];

// Adds per-category frequency data from the spoken-YouTube corpus to the words table.
// Uses a temp table + single UPDATE FROM for performance; safe to call on every startup
// since it detects an already-populated table and skips the import.
pub fn init_frequency(db_path: &str, freq_path: &str) {
    let conn = Connection::open(db_path).expect("failed to open DB");

    // Migrate: add freq columns if absent (silently ignored when already present)
    for col in FREQ_COLS {
        let _ = conn.execute_batch(&format!(
            "ALTER TABLE words ADD COLUMN {col} INTEGER NOT NULL DEFAULT 0"
        ));
    }

    // The word-lookup index doubles as "frequency already imported" marker
    let already_done: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_words_word'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if already_done {
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM words WHERE freq_total > 0", [], |r| r.get(0))
            .unwrap_or(0);
        println!("Frequency data already loaded ({} words matched), skipping.", n);
        return;
    }

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_words_word ON words(word)",
    )
    .expect("failed to create word index");

    conn.execute_batch(
        "CREATE TEMP TABLE freq_data (
            word             TEXT PRIMARY KEY,
            freq_total       INTEGER NOT NULL DEFAULT 0,
            freq_howto       INTEGER NOT NULL DEFAULT 0,
            freq_science     INTEGER NOT NULL DEFAULT 0,
            freq_entertainment INTEGER NOT NULL DEFAULT 0,
            freq_education   INTEGER NOT NULL DEFAULT 0,
            freq_people      INTEGER NOT NULL DEFAULT 0,
            freq_music       INTEGER NOT NULL DEFAULT 0,
            freq_autos       INTEGER NOT NULL DEFAULT 0,
            freq_comedy      INTEGER NOT NULL DEFAULT 0,
            freq_film        INTEGER NOT NULL DEFAULT 0,
            freq_gaming      INTEGER NOT NULL DEFAULT 0,
            freq_sports      INTEGER NOT NULL DEFAULT 0,
            freq_news        INTEGER NOT NULL DEFAULT 0,
            freq_nonprofits  INTEGER NOT NULL DEFAULT 0,
            freq_travel      INTEGER NOT NULL DEFAULT 0,
            freq_pets        INTEGER NOT NULL DEFAULT 0
        )",
    )
    .expect("failed to create freq_data temp table");

    let content = fs::read_to_string(freq_path).expect("failed to read frequency file");
    println!("Loading frequency data...");

    conn.execute_batch("BEGIN").expect("begin");
    let mut stmt = conn
        .prepare(
            "INSERT OR IGNORE INTO freq_data VALUES \
             (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
        )
        .expect("failed to prepare freq insert");

    let mut count = 0u32;
    for line in content.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 19 {
            continue;
        }
        let p = |i: usize| -> i64 { cols[i].parse().unwrap_or(0) };
        stmt.execute(params![
            cols[0],
            p(1),  // total
            p(4),  // howto
            p(5),  // science
            p(6),  // entertainment
            p(7),  // education
            p(8),  // people
            p(9),  // music
            p(10), // autos
            p(11), // comedy
            p(12), // film
            p(13), // gaming
            p(14), // sports
            p(15), // news
            p(16), // nonprofits
            p(17), // travel
            p(18), // pets
        ])
        .expect("failed to insert freq entry");
        count += 1;
        if count % 50_000 == 0 {
            conn.execute_batch("COMMIT; BEGIN").expect("batch commit");
            println!("  {} entries...", count);
        }
    }
    conn.execute_batch("COMMIT").expect("final commit");
    println!("  {} frequency entries loaded", count);

    println!("Matching frequencies to dictionary...");
    conn.execute_batch(
        "UPDATE words
         SET
             freq_total         = f.freq_total,
             freq_howto         = f.freq_howto,
             freq_science       = f.freq_science,
             freq_entertainment = f.freq_entertainment,
             freq_education     = f.freq_education,
             freq_people        = f.freq_people,
             freq_music         = f.freq_music,
             freq_autos         = f.freq_autos,
             freq_comedy        = f.freq_comedy,
             freq_film          = f.freq_film,
             freq_gaming        = f.freq_gaming,
             freq_sports        = f.freq_sports,
             freq_news          = f.freq_news,
             freq_nonprofits    = f.freq_nonprofits,
             freq_travel        = f.freq_travel,
             freq_pets          = f.freq_pets
         FROM freq_data f
         WHERE words.word = f.word",
    )
    .expect("failed to update word frequencies");

    let matched: i64 = conn
        .query_row("SELECT COUNT(*) FROM words WHERE freq_total > 0", [], |r| r.get(0))
        .unwrap_or(0);
    println!("Frequency import complete: {} words matched in dictionary", matched);
}

pub struct WordEntry {
    pub lemma: String,
    pub word_kana: String,
    // TODO: multiple meanings per language — currently joined with ';' as secondary delimiter.
    // Future consideration: how do we know which meaning fits the context?
    pub translations_de: Vec<String>,
    pub translations_en: Vec<String>,
}

// Looks up each lemma in the DB and returns word_kana + translations for 'de' and 'en'.
// Words not found in JMdict are still returned with empty translation fields.
// lookup_words uses prepare_cached inside the loop — rusqlite caches the compiled statement and reuses it for all 15
// lookups instead of recompiling each time.
pub fn lookup_words(db_path: &str, lemmas: &[&str]) -> Vec<WordEntry> {
    let conn = Connection::open(db_path).expect("failed to open DB");

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
                    translations_de: Vec::new(),
                    translations_en: Vec::new(),
                };
            };

            let mut stmt = conn
                .prepare_cached(
                    "SELECT language, meaning FROM glosses
                     WHERE word_id = ?1 AND language IN ('de', 'en')",
                )
                .expect("failed to prepare gloss query");

            let mut translations_de = Vec::new();
            let mut translations_en = Vec::new();

            stmt.query_map(params![word_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("gloss query failed")
            .filter_map(|r| r.ok())
            .for_each(|(lang, meaning)| match lang.as_str() {
                "de" => translations_de.push(meaning),
                "en" => translations_en.push(meaning),
                _ => {}
            });

            WordEntry {
                lemma: lemma.to_string(),
                word_kana,
                translations_de,
                translations_en,
            }
        })
        .collect()
}

// Returns all words marked is_known = 1, as a flat HashSet of both their kanji
// and kana forms so callers can do a single .contains() check against any word form.
pub fn load_known_words(db_path: &str) -> HashSet<String> {
    let conn = Connection::open(db_path).expect("failed to open DB");
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

// Resets all is_known flags and marks DB rows matching the provided word set.
// Matches on both `word` (kanji) and `word_kana` (hiragana) to cover all input forms.
// TODO/TOTEST: What if only kana and too many words get updated? This is a hard one to figure out
pub fn sync_known_words(db_path: &str, words: &HashSet<String>) {
    let conn = Connection::open(db_path).expect("failed to open DB");
    conn.execute_batch("UPDATE words SET is_known = 0").expect("failed to reset is_known");

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
        "Known words: {} entries, {} DB rows marked",
        words.len(),
        matched
    );
}

// Katakana (U+30A1–U+30F6) → Hiragana by subtracting 0x60
// TODO / TOTEST: whats with the chou-on?
fn katakana_to_hiragana(s: &str) -> String {
    s.chars()
        .map(|c| {
            if ('\u{30A1}'..='\u{30F6}').contains(&c) {
                char::from_u32(c as u32 - 0x60).unwrap_or(c)
            } else {
                c
            }
        })
        .collect()
}

// Map JMdict's 3-letter ISO codes to 2-letter; default (no attribute) → "en"
// TODO: Check if needed. Actually the same codes as in jmdict can be used. Its nice to have it documented tho
fn map_lang(raw: &str) -> &str {
    match raw {
        "ger" => "de",
        "fre" | "fra" => "fr",
        "dut" | "nld" => "nl",
        "spa" => "es",
        "hun" => "hu",
        "rus" => "ru",
        "slv" => "sl",
        "eng" => "en",
        other => other,
    }
}

#[derive(PartialEq)]
enum State {
    Root,
    Entry,
    EntrySeq,
    KEle,
    KEleKeb,
    REle,
    REleReb,
    Sense,
    SensePos,
    SenseGloss(String),
}

fn parse_and_insert(conn: &Connection, path: &str) {
    let mut reader = Reader::from_file(path).expect("failed to open JMdict");
    reader.config_mut().trim_text(true);

    let mut state = State::Root;
    let mut buf = Vec::new();

    let mut entry_id: i64 = 0;
    let mut kanji_entries: Vec<String> = Vec::new();
    let mut kana_entries: Vec<String> = Vec::new();
    let mut pos_set: Vec<String> = Vec::new();
    let mut glosses: Vec<(String, String)> = Vec::new();
    let mut count = 0u32;

    conn.execute_batch("BEGIN").expect("begin transaction");

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.name().as_ref() {
                b"entry" if state == State::Root => {
                    state = State::Entry;
                    entry_id = 0;
                    kanji_entries.clear();
                    kana_entries.clear();
                    pos_set.clear();
                    glosses.clear();
                }
                b"ent_seq" if state == State::Entry => state = State::EntrySeq,
                b"k_ele" if state == State::Entry => state = State::KEle,
                b"keb" if state == State::KEle => state = State::KEleKeb,
                b"r_ele" if state == State::Entry => state = State::REle,
                b"reb" if state == State::REle => state = State::REleReb,
                b"sense" if state == State::Entry => state = State::Sense,
                b"pos" if state == State::Sense => state = State::SensePos,
                b"gloss" if state == State::Sense => {
                    let lang = e
                        .attributes()
                        .filter_map(|a| a.ok())
                        .find(|a| a.key.as_ref() == b"xml:lang")
                        .map(|a| {
                            let raw = std::str::from_utf8(a.value.as_ref()).unwrap_or("en");
                            map_lang(raw).to_string()
                        })
                        .unwrap_or_else(|| "en".to_string());
                    state = State::SenseGloss(lang);
                }
                _ => {}
            },
            Ok(Event::End(e)) => match e.name().as_ref() {
                b"entry" => {
                    let word = kanji_entries
                        .first()
                        .cloned()
                        .or_else(|| kana_entries.first().cloned())
                        .unwrap_or_default();
                    let word_kana = kana_entries
                        .first()
                        .map(|s| katakana_to_hiragana(s))
                        .unwrap_or_default();
                    let pos = if pos_set.is_empty() {
                        None
                    } else {
                        Some(pos_set.join(","))
                    };

                    conn.execute(
                        "INSERT OR IGNORE INTO words (id, word, word_kana, pos) VALUES (?1, ?2, ?3, ?4)",
                        params![entry_id, word, word_kana, pos],
                    ).expect("insert word failed");

                    for (lang, meaning) in &glosses {
                        conn.execute(
                            "INSERT INTO glosses (language, meaning, word_id) VALUES (?1, ?2, ?3)",
                            params![lang, meaning, entry_id],
                        ).expect("insert gloss failed");
                    }

                    count += 1;
                    if count % 10_000 == 0 {
                        conn.execute_batch("COMMIT").expect("commit");
                        conn.execute_batch("BEGIN").expect("begin");
                        println!("  {} entries imported...", count);
                    }
                    state = State::Root;
                }
                b"ent_seq" => state = State::Entry,
                b"k_ele" => state = State::Entry,
                b"keb" => state = State::KEle,
                b"r_ele" => state = State::Entry,
                b"reb" => state = State::REle,
                b"sense" => state = State::Entry,
                b"pos" => state = State::Sense,
                b"gloss" => state = State::Sense,
                _ => {}
            },
            Ok(Event::Text(e)) => match &state {
                State::EntrySeq => {
                    if let Ok(text) = e.unescape() {
                        entry_id = text.trim().parse().unwrap_or(0);
                    }
                }
                State::KEleKeb => {
                    if let Ok(text) = e.unescape() {
                        kanji_entries.push(text.into_owned());
                    }
                }
                State::REleReb => {
                    if let Ok(text) = e.unescape() {
                        kana_entries.push(text.into_owned());
                    }
                }
                State::SensePos => {
                    // <pos> contains XML entity refs like &n; or &adj-na;
                    // quick-xml doesn't expand DTD entities, so read raw bytes
                    let raw = std::str::from_utf8(e.as_ref()).unwrap_or("").trim();
                    let code = if raw.starts_with('&') && raw.ends_with(';') {
                        &raw[1..raw.len() - 1]
                    } else {
                        raw
                    };
                    if !code.is_empty() && !pos_set.iter().any(|p| p == code) {
                        pos_set.push(code.to_string());
                    }
                }
                State::SenseGloss(lang) => {
                    if let Ok(text) = e.unescape() {
                        let meaning = text.trim().to_string();
                        if !meaning.is_empty() {
                            glosses.push((lang.clone(), meaning));
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                eprintln!("XML parse error: {}", e);
                break;
            }
            _ => {}
        }
    }

    conn.execute_batch("COMMIT").expect("final commit");
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_glosses_word_id ON glosses(word_id)"
    ).expect("create index");

    println!("Dictionary import complete: {} entries", count);
}
