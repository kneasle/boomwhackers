#!/usr/bin/env bash
TEMP_PATH=./boomwhackers

mkdir -p $TEMP_PATH # Make temp files

cargo run --release -- $1 $TEMP_PATH # Determine whacker assignments and build MusicXML files
musescore3 -j $TEMP_PATH/jobs.json # Build all the MusicXML files into PDFs
pdftk $TEMP_PATH/*.pdf cat output $2 # Combine the PDFs

rm -r $TEMP_PATH # Clean up temporary files
