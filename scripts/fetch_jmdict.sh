#!/bin/sh


echo "in case of any trouble, pls check the link on edrdg.org, as well as the filename of the .gz file"

DICT_NAME="JMdict"

echo "Downloading the jmdict from FTP-server: ftp://ftp.edrdg.org/pub/Nihongo/\`${DICT_NAME}\`.gz ..."
echo

wget ftp://ftp.edrdg.org/pub/Nihongo/${DICT_NAME}.gz
gunzip ${DICT_NAME}.gz
mv ${DICT_NAME}.gz ../resources/JMdict.xml
rm -rf ${DICT_NAME}.gz ${DICT_NAME}

echo
echo "Placed a dictionary file to \`../resources/JMdict.xml\` ."
