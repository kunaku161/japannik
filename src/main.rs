mod cli;
mod config;
mod db;
mod dictionary;
mod frequency;
mod known_words;
mod normalizer;
mod words;

use config::{Config, SudachiMode};
use normalizer::Normalizer;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let config = Config::load_or_default("japannik.toml");

    db::ensure_schema(&config.paths.db);
    dictionary::init_dictionary(&config.paths.db, &config.paths.jmdict);
    frequency::init_frequency(&config.paths.db, &config.paths.frequency);

    // Normalizer is expensive to construct — create once and reuse across actions.
    let mut normalizer = Normalizer::new(
        Some(PathBuf::from(&config.paths.sudachi_dict)),
        config.general.sudachi_mode,
    );

    cli::show_welcome();

    loop {
        match cli::main_menu() {
            cli::Action::GenerateWords   => action_generate_words(&config, &mut normalizer),
            cli::Action::EatForSentences => println!("\n  Sentence mining coming soon!\n"),
            cli::Action::AddKnownWords   => action_add_known_words(&config, &mut normalizer),
            cli::Action::ResetKnownWords => action_reset_known_words(&config),
            cli::Action::Config          => action_show_config(&config),
            cli::Action::Quit            => { println!("\n  またね！\n"); break; }
        }
    }
}

// ── Actions ───────────────────────────────────────────────────────────────────

fn action_generate_words(config: &Config, normalizer: &mut Normalizer) {
    let source = cli::select_input_source("input");
    let text = read_text_from_source(&source);
    if text.is_empty() {
        println!("  No text found — aborting.\n");
        return;
    }
    println!("  Read {} characters", text.chars().count());

    let known_words = if config.general.ignore_known_words {
        words::load_known_words(&config.paths.db)
    } else {
        std::collections::HashSet::new()
    };

    let morphemes = normalizer.normalize(&text);

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

    // Sort all unknown lemmas by in-text occurrence count (highest first)
    let mut all_unknown = hashmap_to_sorted_vec(&lemma_counts);
    all_unknown.retain(|(_, count)| *count >= config.output.min_text_occurrences);

    if all_unknown.is_empty() {
        println!("  No unknown words found.\n");
        return;
    }

    // Pool A: top max_from_text by text occurrence
    let pool_a_end = config.output.max_from_text.min(all_unknown.len());
    let pool_a: Vec<(&str, usize)> = all_unknown[..pool_a_end].to_vec();
    let pool_a_set: std::collections::HashSet<&str> = pool_a.iter().map(|(w, _)| *w).collect();

    // Pool B: remaining text words re-ranked by corpus frequency + bias
    let pool_b_candidates: Vec<&str> = all_unknown[pool_a_end..]
        .iter()
        .map(|(w, _)| *w)
        .filter(|w| !pool_a_set.contains(*w))
        .collect();

    let mut pool_b_scored =
        words::score_lemmas(&config.paths.db, &pool_b_candidates, &config.frequency_bias);
    pool_b_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let pool_b_end = config.output.max_from_corpus.min(pool_b_scored.len());
    let pool_b: Vec<&str> = pool_b_scored[..pool_b_end]
        .iter()
        .map(|(w, _)| w.as_str())
        .collect();

    println!(
        "\n  Pool A — {} words by text frequency:  {:?}",
        pool_a.len(),
        pool_a
    );
    println!(
        "  Pool B — {} words by corpus frequency: {:?}\n",
        pool_b.len(),
        pool_b
    );

    let all_lemmas: Vec<&str> = pool_a
        .iter()
        .map(|(w, _)| *w)
        .chain(pool_b.iter().copied())
        .collect();

    let mut word_entries =
        words::lookup_words(&config.paths.db, &all_lemmas, &config.output.output_languages);

    if config.output.output_freq_score {
        let scored = words::score_lemmas(&config.paths.db, &all_lemmas, &config.frequency_bias);
        let score_map: HashMap<&str, f64> =
            scored.iter().map(|(w, s)| (w.as_str(), *s)).collect();
        for entry in &mut word_entries {
            entry.freq_score = score_map.get(entry.lemma.as_str()).copied();
        }
    }

    write_csv_output("output", &word_entries, config);
}

fn action_add_known_words(config: &Config, normalizer: &mut Normalizer) {
    let source = cli::select_known_words_source(&config.paths.known_words_dir);
    let norm_opt = if config.general.normalize_known_words {
        Some(normalizer as &mut Normalizer)
    } else {
        None
    };
    let known_set = match source {
        cli::PathSource::Dir(dir) => known_words::load_from_dir(
            dir.to_str().unwrap_or("known_words"),
            config.general.known_words_col,
            norm_opt,
        ),
        cli::PathSource::File(file) => {
            known_words::load_from_file(&file, config.general.known_words_col, norm_opt)
        }
    };
    words::add_known_words(&config.paths.db, &known_set);
    println!();
}

fn action_reset_known_words(config: &Config) {
    if cli::confirm_reset() {
        words::reset_known_words(&config.paths.db);
    } else {
        println!("  Reset cancelled.");
    }
    println!();
}

fn action_show_config(config: &Config) {
    let mode = match config.general.sudachi_mode {
        SudachiMode::Single   => "single   (Mode A — smallest morpheme units)",
        SudachiMode::Compound => "compound (Mode B — standard dictionary words)",
        SudachiMode::Idioms   => "idioms   (Mode C — keeps set phrases together)",
    };

    println!();
    println!("  Current configuration  (edit japannik.toml to change)");
    println!("  {}", "─".repeat(52));
    println!();
    println!("  [paths]");
    println!("    db              : {}", config.paths.db);
    println!("    jmdict          : {}", config.paths.jmdict);
    println!("    frequency       : {}", config.paths.frequency);
    println!("    known_words_dir : {}", config.paths.known_words_dir);
    println!();
    println!("  [general]");
    println!("    sudachi_mode      : {mode}");
    println!("    ignore_known      : {}", config.general.ignore_known_words);
    println!("    normalize_known   : {}", config.general.normalize_known_words);
    println!("    known_words_col   : {}", config.general.known_words_col);
    println!("    verbose           : {}", config.general.verbose);
    println!();
    println!("  [output]");
    println!("    max_from_text     : {}", config.output.max_from_text);
    println!("    max_from_corpus   : {}", config.output.max_from_corpus);
    println!("    min_occurrences   : {}", config.output.min_text_occurrences);
    println!("    output_languages  : {:?}", config.output.output_languages);
    println!("    output_freq_score : {}", config.output.output_freq_score);
    println!();

    let b = &config.frequency_bias;
    let active: Vec<(&str, f64)> = [
        ("howto", b.howto), ("science", b.science), ("entertainment", b.entertainment),
        ("education", b.education), ("people", b.people), ("music", b.music),
        ("autos", b.autos), ("comedy", b.comedy), ("film", b.film),
        ("gaming", b.gaming), ("sports", b.sports), ("news", b.news),
        ("nonprofits", b.nonprofits), ("travel", b.travel), ("pets", b.pets),
    ]
    .into_iter()
    .filter(|(_, w)| *w != 0.0)
    .collect();

    println!("  [frequency_bias]");
    if active.is_empty() {
        println!("    (all zeros — sorting Pool B by total corpus frequency)");
    } else {
        for (name, weight) in &active {
            println!("    {name:<15}: {weight}");
        }
    }
    println!();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn read_text_from_source(source: &cli::PathSource) -> String {
    match source {
        cli::PathSource::Dir(dir) => fs::read_dir(dir)
            .expect("failed to open input directory")
            .filter_map(|e| {
                let path = e.ok()?.path();
                // TODO: expand to other formats (.srt, etc.)
                if path.extension()?.to_str()? == "txt" {
                    fs::read_to_string(&path).ok()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        cli::PathSource::File(path) => fs::read_to_string(path).unwrap_or_else(|e| {
            eprintln!("  Failed to read {:?}: {}", path, e);
            String::new()
        }),
    }
}

fn hashmap_to_sorted_vec(map: &HashMap<String, usize>) -> Vec<(&str, usize)> {
    let mut entries: Vec<(&str, usize)> = map.iter().map(|(k, &v)| (k.as_str(), v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));
    entries
}

fn write_csv_output(dir: &str, entries: &[words::WordEntry], config: &Config) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();
    let output_path = format!("{dir}/{timestamp}_vocab.csv");

    let mut file = fs::File::create(&output_path).expect("failed to create vocab file");

    let mut header = vec!["word".to_string(), "hiragana".to_string()];
    for lang in &config.output.output_languages {
        header.push(format!("translation_{lang}"));
    }
    if config.output.output_freq_score {
        header.push("freq_score".to_string());
    }
    header.push("example_sentence".to_string());
    writeln!(file, "{}", header.join("|")).expect("failed to write header");

    for entry in entries {
        let mut row = vec![entry.lemma.clone(), entry.word_kana.clone()];
        for lang in &config.output.output_languages {
            row.push(
                entry
                    .translations
                    .get(lang)
                    .map(|v| v.join(";"))
                    .unwrap_or_default(),
            );
        }
        if config.output.output_freq_score {
            row.push(
                entry
                    .freq_score
                    .map(|s| format!("{s:.0}"))
                    .unwrap_or_default(),
            );
        }
        row.push(String::new()); // example_sentence placeholder
        writeln!(file, "{}", row.join("|")).expect("failed to write entry");
    }

    println!("  Saved {} entries to {}", entries.len(), output_path);
}
