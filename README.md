A small MVP to generate a "|" delimited .csv out of some japanese input text to then import it into anki.

```word|hiragana|translation_german|translation_english|example_sentence```

```学校|がっこう|Schule;Lehranstalt;Bildungseinrichtung;Akademie;Seminar|school|```

For now it produces rows for the most occuring words within our input text.

# install

1. you'll need to install rust and cargo
https://rust-lang.github.io/rustup/installation/other.html

2. get resources (JMdict for translation lookup, system.dic for sudachi tokenizer for word normalization)
make the files within /scripts/ executable and run the shell scripts fetch_dictionary.sh and fetch_jmdict.sh
On linux this would look somewhat like this:

```chmod +x filename.sh```

```./filename.sh```

4. build and run cargo
cd to the projects root directory
execute ```cargo build``` to build the binary from the source
execute ```cargo run``` for the initial run. This will read the JMdict.xml and create a .db file with its contents within the root directory

# run

```cargo run``` will (for now) just run all functionality of this program. No interaction. Just examples and testing. 
