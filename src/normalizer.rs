use std::path::PathBuf;
use std::rc::Rc;
use sudachi::analysis::stateful_tokenizer::StatefulTokenizer;
use sudachi::config::Config as SudachiConfig;
use sudachi::dic::dictionary::JapaneseDictionary;
use sudachi::prelude::*;
use crate::config::SudachiMode;

// Rc<JapaneseDictionary> is the idiomatic way to share the dictionary between
// the tokenizer and the morpheme list without cloning the heavy data.
// Rc is single-threaded reference counting — fine since we never send this across threads.
pub struct Normalizer {
    tokenizer: StatefulTokenizer<Rc<JapaneseDictionary>>,
    morpheme_list: MorphemeList<Rc<JapaneseDictionary>>,
}

impl Normalizer {
    pub fn new(dict_path: Option<PathBuf>, mode: SudachiMode) -> Self {
        let sudachi_mode = match mode {
            SudachiMode::Single   => Mode::A,
            SudachiMode::Compound => Mode::B,
            SudachiMode::Idioms   => Mode::C,
        };
        let config = SudachiConfig::new(None, None, dict_path).expect("failed to load sudachi config");
        let dictionary = Rc::new(
            JapaneseDictionary::from_cfg(&config).expect("failed to load dictionary"),
        );
        Self {
            tokenizer: StatefulTokenizer::create(dictionary.clone(), false, sudachi_mode),
            morpheme_list: MorphemeList::empty(dictionary),
        }
    }

    // Tokenizes `text` and returns a reference to the internal morpheme list.
    // Takes &mut self because the tokenizer state is mutated on each call.
    // The returned reference is valid until the next call to normalize().
    pub fn normalize(&mut self, text: &str) -> &MorphemeList<Rc<JapaneseDictionary>> {
        self.tokenizer.reset().push_str(text);
        self.tokenizer.do_tokenize().expect("tokenization failed");
        self.morpheme_list
            .collect_results(&mut self.tokenizer)
            .expect("failed to collect morpheme results");
        &self.morpheme_list
    }
}
