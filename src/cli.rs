use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::fs;
use std::path::PathBuf;

// ── Logo ──────────────────────────────────────────────────────────────────────

const LOGO: &str = r#"
    ....:::::::::.  ::::::::::. :::.   :::.    :::.:::.    :::.::: :::  .
 ;;;;;;;;;````;;`;;  `;;;```.;;;;;`;;  `;;;;,  `;;;`;;;;,  `;;;;;; ;;; .;;,.
 ''`  `[[.   ,[[ '[[, `]]nnn]]',[[ '[[,  [[[[[. '[[  [[[[[. '[[[[[ [[[[[/'
,,,    `$$  c$$$cc$$$c $$$""  c$$$cc$$$c $$$ "Y$c$$  $$$ "Y$c$$$$$_$$$$,
888boood88   888   888,888o    888   888,888    Y88  888    Y88888"888"88o,
"MMMMMMMM"   YMM   ""` YMMMb   YMM   ""` MMM     YM  MMM     YMMMM MMM "MMP"
"#;

const CYAN: &str = "\x1b[96m";
const RESET: &str = "\x1b[0m";

pub fn show_welcome() {
    println!("{}{}{}", CYAN, LOGO, RESET);
    println!("  Damn, Tung! \n");
}

// ── Actions ───────────────────────────────────────────────────────────────────

pub enum Action {
    GenerateWords,
    EatForSentences,
    AddKnownWords,
    ResetKnownWords,
    Config,
    Quit,
}

pub fn main_menu() -> Action {
    let options = &[
        "1  Generate words to learn from input",
        "2  Eat input for sentences          (coming soon)",
        "3  Add known words",
        "4  Reset known words",
        "5  Config",
        "6  Quit",
    ];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select action")
        .items(options)
        .default(0)
        .interact()
        .expect("failed to read menu selection");

    match selection {
        0 => Action::GenerateWords,
        1 => Action::EatForSentences,
        2 => Action::AddKnownWords,
        3 => Action::ResetKnownWords,
        4 => Action::Config,
        5 => Action::Quit,
        _ => Action::Quit,
    }
}

// ── Input / path selection ────────────────────────────────────────────────────

/// A resolved input source: a directory (→ read all .txt files) or a single file.
pub enum PathSource {
    Dir(PathBuf),
    File(PathBuf),
}

/// Prompts the user to pick an input source from the `input/` directory.
pub fn select_input_source(input_dir: &str) -> PathSource {
    let options = &[
        "All files in input/  (default)",
        "Pick a specific file from input/",
        "Enter a custom path (file or directory)",
    ];
    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Input source")
        .items(options)
        .default(0)
        .interact()
        .expect("failed to read input selection");

    match choice {
        0 => PathSource::Dir(PathBuf::from(input_dir)),
        1 => {
            let files = list_txt_files(input_dir);
            if files.is_empty() {
                println!("  No .txt files found in {input_dir} — using directory as fallback.");
                return PathSource::Dir(PathBuf::from(input_dir));
            }
            let names: Vec<String> = files
                .iter()
                .map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned())
                .collect();
            let idx = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select file")
                .items(&names)
                .default(0)
                .interact()
                .expect("failed to read file selection");
            PathSource::File(files[idx].clone())
        }
        2 => {
            let raw: String = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Path (file or directory)")
                .interact_text()
                .expect("failed to read path");
            let p = PathBuf::from(raw.trim());
            if p.is_dir() { PathSource::Dir(p) } else { PathSource::File(p) }
        }
        _ => PathSource::Dir(PathBuf::from(input_dir)),
    }
}

/// Prompts the user to pick a known-words source from `known_words/`.
pub fn select_known_words_source(dir: &str) -> PathSource {
    let files = list_txt_files(dir);

    let mut options: Vec<String> = vec![format!("All files in {dir}/  (add all)")];
    options.extend(
        files
            .iter()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().into_owned()),
    );
    options.push("Enter a custom path".to_string());

    let choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Known words source")
        .items(&options)
        .default(0)
        .interact()
        .expect("failed to read known words selection");

    if choice == 0 {
        PathSource::Dir(PathBuf::from(dir))
    } else if choice == options.len() - 1 {
        let raw: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Path (file or directory)")
            .interact_text()
            .expect("failed to read path");
        let p = PathBuf::from(raw.trim());
        if p.is_dir() { PathSource::Dir(p) } else { PathSource::File(p) }
    } else {
        PathSource::File(files[choice - 1].clone())
    }
}

// ── Confirmations ─────────────────────────────────────────────────────────────

pub fn confirm_reset() -> bool {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Remove ALL known-word flags from the database? This cannot be undone.")
        .default(false)
        .interact()
        .unwrap_or(false)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn list_txt_files(dir: &str) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |ext| ext == "txt"))
                .collect()
        })
        .unwrap_or_default()
}
