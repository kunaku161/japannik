mod db;
mod known_words;
mod normalizer;

use normalizer::Normalizer;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LEMMAS: usize = 15;
// Set to true to also add the sudachi dictionary form of each known word,
// which catches inflected forms (e.g. 構わない → also marks 構う as known).
const NORMALIZE_KNOWN_WORDS: bool = true;

fn main() {
    db::init_dictionary("japannik.db", "resources/JMdict");
    db::init_frequency("japannik.db", "resources/frequency_spoken.tsv");

    let mut normalizer = Normalizer::new(Some(PathBuf::from("resources/system.dic")));

    let known_set = known_words::load_from_dir(
        "known_words",
        0,
        if NORMALIZE_KNOWN_WORDS { Some(&mut normalizer) } else { None },
    );
    db::sync_known_words("japannik.db", &known_set);
    let known_words = db::load_known_words("japannik.db");

    let text = read_text_from_input("input");
    println!("Read {} characters from input files", text.chars().count());

    let morphemes = normalizer.normalize(&text);

    // pos to be skipped — see pos_notes.txt; subject to expansion
    let skip_pos = ["助動詞", "補助記号", "助詞", "空白", "数詞", "固有名詞", "接尾辞"];
    let mut lemma_counts: HashMap<String, usize> = HashMap::new();

    for morpheme in morphemes.iter() {
        let pos = morpheme.part_of_speech();
        if pos.iter().any(|p| skip_pos.contains(&p.as_str())) {
            continue;
        }
        let lemma = morpheme.dictionary_form();
        if known_words.contains(lemma) {
            continue;
        }
        *lemma_counts.entry(lemma.to_string()).or_insert(0) += 1;
    }

    // Count → sort → take top N (can't cut earlier — need all counts to rank correctly)
    let sorted_lemmas = hashmap_to_sorted_vec(&lemma_counts);
    let top = &sorted_lemmas[..MAX_LEMMAS.min(sorted_lemmas.len())];
    println!("{:?}", top);

    let lemma_words: Vec<&str> = top.iter().map(|(w, _)| *w).collect();
    let word_entries = db::lookup_words("japannik.db", &lemma_words);
    write_csv_output("output", &word_entries);

    write_output("output", top);
}





// lines to visually seperate from fn main




// Reads all .txt files from `dir` and joins their contents with a newline.
fn read_text_from_input(dir: &str) -> String {
    fs::read_dir(dir)
        .expect("failed to open input folder")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            // TODO: expand to other text formats (.srt, etc.) as needed
            if path.extension()?.to_str()? == "txt" {
                fs::read_to_string(&path).ok()
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Sorts lemma counts by frequency (highest first) and returns (lemma, count) pairs.
fn hashmap_to_sorted_vec(map: &HashMap<String, usize>) -> Vec<(&str, usize)> {
    let mut entries: Vec<(&str, usize)> = map.iter().map(|(k, &v)| (k.as_str(), v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
}

// Writes pipe-delimited vocab rows to `{dir}/{timestamp}_vocab.csv`.
// Multiple meanings within a language are joined with ';' as a secondary delimiter.
// TODO: Sentence Crawler — example_sentence column is a placeholder for now.
fn write_csv_output(dir: &str, entries: &[db::WordEntry]) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();
    let output_path = format!("{}/{}_vocab.csv", dir, timestamp);

    let mut file = fs::File::create(&output_path).expect("failed to create vocab file");
    writeln!(file, "word|hiragana|translation_german|translation_english|example_sentence")
        .expect("failed to write header");

    for entry in entries {
        writeln!(
            file,
            "{}|{}|{}|{}|",
            entry.lemma,
            entry.word_kana,
            entry.translations_de.join(";"),
            entry.translations_en.join(";"),
        )
        .expect("failed to write entry");
    }

    println!("Saved {} vocab entries to {}", entries.len(), output_path);
}

// Writes lemmas (one per line) to `{dir}/{timestamp}_output.csv`.
fn write_output(dir: &str, sorted_lemmas: &[(&str, usize)]) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();
    let output_path = format!("{}/{}_output.csv", dir, timestamp);

    let mut file = fs::File::create(&output_path).expect("failed to create output file");
    for (lemma, _) in sorted_lemmas {
        writeln!(file, "{}", lemma).expect("failed to write lemma");
    }

    println!("Saved {} lemmas to {}", sorted_lemmas.len(), output_path);
}
