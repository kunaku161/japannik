use quick_xml::events::Event;
use quick_xml::Reader;
use rusqlite::{params, Connection};
use crate::db;

pub fn init_dictionary(db_path: &str, jmdict_path: &str) {
    let conn = db::open(db_path);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM words", [], |row| row.get(0))
        .unwrap_or(0);
    if count > 0 {
        println!("Dictionary already initialized ({} words), skipping import.", count);
        return;
    }
    parse_and_insert(&conn, jmdict_path);
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
    println!("Dictionary import complete: {} entries", count);
}
