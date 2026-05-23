#!/bin/sh

DIR="../resources/frequency_spoken.tsv"
FILE="tubelex-ja.tsv"
URL="https://github.com/naist-nlp/tubelex/raw/refs/heads/main/frequencies/tubelex-ja.tsv.xz"

echo "Downloading tubelex-ja frequency list..."

curl -L "$URL" -o "${FILE}.xz"

echo "Extracting file via xz (xz-utils)..."

xz -dk "${FILE}.xz"

echo "Clean up..."

mv ${FILE} ${DIR}
rm -rf ${FILE}.xz

echo "done"



