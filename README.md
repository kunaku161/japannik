A small MVP to generate a "|" delimited .csv out of some japanese input text to then import it into anki.

```word|hiragana|translation_german|translation_english|example_sentence```

```学校|がっこう|Schule;Lehranstalt;Bildungseinrichtung;Akademie;Seminar|school|```

For now it produces rows for the most occuring words within our input text. Also if a word does not occur often, but is occuring in daily japanese every so often, it will also appear in the list, if configured.
For example: In the first few episodes of One Piece, they will say the word 海賊 (Pirates) a couple of hundred times. So you should learn it.
The word 飲む occurs just once. But it's so common. You should learn it either way.

# install

1. you'll need to install rust and cargo
https://rust-lang.github.io/rustup/installation/other.html

2. get resources (JMdict for translation lookup, system.dic for sudachi normalizer, tubelex-ja for frequency)
make the files within /scripts/ executable and run the shell scripts fetch_dictionary.sh, fetch_jmdict.sh and fetch_frequency_spoken.sh
On linux this would look somewhat like this:

```chmod +x filename.sh```

```./filename.sh```

4. build and run cargo
cd to the projects root directory
execute ```cargo build``` to build the binary from the source
execute ```cargo run``` for the initial run. This will read the JMdict.xml and create a .db file with its contents within the root directory

# run

```cargo run``` will run an mvp CLI Tool to test out the funcionalities 
