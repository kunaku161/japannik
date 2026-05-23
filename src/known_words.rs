use std::collections::HashSet;
use std::fs;
use std::path::Path;
use crate::normalizer::Normalizer;

// Scans `dir` for .txt files, extracts the word from `col` (0-based), cleans each field,
// and optionally expands each word by its sudachi dictionary form(s).
pub fn load_from_dir(dir: &str, col: usize, normalizer: Option<&mut Normalizer>) -> HashSet<String> {
    let mut words: HashSet<String> = HashSet::new();
    let mut file_count = 0;

    let entries = fs::read_dir(dir).expect("failed to open known_words directory");
    println!("Importing known words...");

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let raw = read_anki_file(&path, col);
        println!("  {}: {} words", path.file_name().unwrap_or_default().to_string_lossy(), raw.len());
        words.extend(raw);
        file_count += 1;
    }

    println!("Found {} files, {} unique words", file_count, words.len());

    if let Some(norm) = normalizer {
        let originals: Vec<String> = words.iter().cloned().collect();
        let mut added = 0usize;
        for word in &originals {
            for m in norm.normalize(word).iter() {
                let dict_form: String = m.dictionary_form().chars().filter(|&c| is_japanese(c)).collect();
                if !dict_form.is_empty() && words.insert(dict_form) {
                    added += 1;
                }
            }
        }
        println!("After normalization: +{} forms, {} total", added, words.len());
    }

    words
}

/// Imports a single Anki TSV file instead of the whole directory.
pub fn load_from_file(path: &Path, col: usize, normalizer: Option<&mut Normalizer>) -> HashSet<String> {
    let mut words: HashSet<String> = HashSet::new();
    let raw = read_anki_file(path, col);
    println!(
        "  {}: {} words",
        path.file_name().unwrap_or_default().to_string_lossy(),
        raw.len()
    );
    words.extend(raw);

    if let Some(norm) = normalizer {
        let originals: Vec<String> = words.iter().cloned().collect();
        let mut added = 0usize;
        for word in &originals {
            for m in norm.normalize(word).iter() {
                let dict_form: String =
                    m.dictionary_form().chars().filter(|&c| is_japanese(c)).collect();
                if !dict_form.is_empty() && words.insert(dict_form) {
                    added += 1;
                }
            }
        }
        if added > 0 {
            println!("After normalization: +{} forms, {} total", added, words.len());
        }
    }

    words
}

fn read_anki_file(path: &Path, col: usize) -> Vec<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|line| {
            let field = line.split('\t').nth(col)?;
            let word = clean_field(field);
            if word.is_empty() { None } else { Some(word) }
        })
        .collect()
}

// Strips Anki pitch/furigana notation and keeps only Japanese characters.
//
// Handles:
//   {[護衛|ごえい];0}  → 護衛   ({...;N} keeps before ';', [A|B] keeps A)
//   {ずばり;2}         → ずばり
//   地理[ちり;a]       → 地理   ([;x] without pipe is discarded)
//   わたし[;h]         → わたし
fn clean_field(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();

    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let mut inner = String::new();
                let mut depth = 1usize;
                for c2 in chars.by_ref() {
                    match c2 {
                        '{' => { depth += 1; inner.push(c2); }
                        '}' => {
                            depth -= 1;
                            if depth == 0 { break; }
                            inner.push(c2);
                        }
                        _ => inner.push(c2),
                    }
                }
                let content = inner.split(';').next().unwrap_or(&inner);
                out.push_str(&clean_field(content));
            }
            '[' => {
                let mut inner = String::new();
                let mut depth = 1usize;
                for c2 in chars.by_ref() {
                    match c2 {
                        '[' => { depth += 1; inner.push(c2); }
                        ']' => {
                            depth -= 1;
                            if depth == 0 { break; }
                            inner.push(c2);
                        }
                        _ => inner.push(c2),
                    }
                }
                if let Some(pipe) = inner.find('|') {
                    out.push_str(&clean_field(&inner[..pipe]));
                }
                // [;x] or [,x] style metadata: discard
            }
            '(' => {
                let mut depth = 1usize;
                for c2 in chars.by_ref() {
                    match c2 {
                        '(' => depth += 1,
                        ')' => { depth -= 1; if depth == 0 { break; } }
                        _ => {}
                    }
                }
            }
            c => out.push(c),
        }
    }

    out.chars().filter(|&c| is_japanese(c)).collect()
}

fn is_japanese(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{30FF}' |  // Hiragana + Katakana (incl. chouon ー U+30FC)
        '\u{4E00}'..='\u{9FFF}' |  // CJK Unified Ideographs
        '\u{3400}'..='\u{4DBF}'    // CJK Extension A
    )
}
