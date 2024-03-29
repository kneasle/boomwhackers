//! Code for loading/modifying/saving MusicXML files.

use std::{
    cmp::Reverse,
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    io::{Cursor, Read},
    path::Path,
    time::Duration,
};

use anyhow::Context;
use itertools::Itertools;
use ordered_float::OrderedFloat;

use crate::note::Note;

/// Representation of a loaded MusicXML file.
#[derive(Debug)]
pub struct MusicXmlScore {
    tree: elementtree::Element,
    pub whacks: HashMap<Note, Vec<Whack>>, // TODO: Not pub
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Whack {
    pub timestamp: Timestamp,
    /// The 0-based index of the `<note>` tag in the source XML `tree` (i.e. if `note_idx = 5`,
    /// then there are 5 `<note>` tags before the one representing this `Whack`).
    note_idx: usize,
    /// The `note_idx` of the first `<note>` in the chord containing this `Whack`
    chord_note_idx: usize,
}

///////////////////
// READING FILES //
///////////////////

impl MusicXmlScore {
    /// Load a `MusicXmlScore` from a file.
    pub fn load_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let mut raw_bytes = Vec::new();
        File::open(&path)
            .context(format!("Error loading {path:?}"))?
            .read_to_end(&mut raw_bytes)
            .context(format!("Error reading {path:?}"))?;
        let extension = path
            .extension()
            .context("Can't read a file with no extension")?;
        Self::from_raw_bytes(&raw_bytes, extension)
    }

    /// Reads a `MusicXmlScore` from some bytes, using the given `extension` to determine whether
    /// or not those bytes are compressed.
    pub fn from_raw_bytes(bytes: &[u8], extension: &OsStr) -> anyhow::Result<Self> {
        let mut decompressed_bytes = Vec::new();
        let xml_bytes = match extension.to_str() {
            Some("xml") => bytes, // No decompression necessary
            Some("mxl") => {
                let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
                    .context("Error extracting the zip archive")?;
                let xml_file_name = archive
                    .file_names()
                    .find(|f| !f.contains('/')) // First file in the root directory of the archive
                    .context("MusicXML archive should have at least one file")?
                    .to_owned();
                let mut xml_file = archive
                    .by_name(&xml_file_name)
                    .context("MusicXML file not found in the archive")?;
                xml_file.read_to_end(&mut decompressed_bytes).unwrap();
                &decompressed_bytes
            }
            _ => {
                return Err(anyhow::Error::msg(format!(
                    "Unknown file extension {extension:?} for MusicXML."
                )));
            }
        };
        Self::from_xml_bytes(xml_bytes)
    }

    /// Read a `MusicXmlScore` from bytes of XML (which may have been uncompressed from the file).
    fn from_xml_bytes(xml_bytes: &[u8]) -> anyhow::Result<Self> {
        let tree =
            elementtree::Element::from_reader(xml_bytes).context("File contains invalid XML")?;
        Ok(Self {
            whacks: load_whacks(&tree)?,
            tree,
        })
    }
}

/// Walk a tree of XML [`Element`](elementtree::Element)s and determine at what times each note is
/// played.
fn load_whacks(tree: &elementtree::Element) -> anyhow::Result<HashMap<Note, Vec<Whack>>> {
    let mut whacks = HashMap::<Note, Vec<Whack>>::new();

    // Stores `(<duration of new bpm>, <new bpm>)`
    let mut bpm_changes = Vec::<(Timestamp, f64)>::new();
    let mut whacks_loaded_so_far = 0;
    for (part_idx, part) in tree.find_all("part").enumerate() {
        // MusicXML expresses all its note values as an integer multiple of some 'division' value
        // (presumably to avoid floating point errors).  For each part, this is stored in the
        // `attributes/divisions` element of the first measure.
        let divs_per_beat = divisions_per_beat(part).ok_or_else(|| {
            anyhow::Error::msg(format!(
                "Couldn't load 'divisions' for part {}",
                part_idx + 1
            ))
        })?;

        // Extract the note names
        let mut current_chord_start = Timestamp::ZERO;
        let mut current_chord_note_idx = whacks_loaded_so_far;
        let mut next_chord_start = Timestamp::ZERO;
        for (measure_idx, measure) in part.children().enumerate() {
            let measure_name = format!("measure {} of part {}", measure_idx + 1, part_idx + 1);
            assert_eq!(measure.tag().name(), "measure");

            for elem in measure.children() {
                match elem.tag().name() {
                    // Extract bpm changes from `direction` elements
                    "direction" => {
                        if let Some(sound_elem) = elem.find("sound") {
                            if let Some(tempo_str) = sound_elem.get_attr("tempo") {
                                let new_bpm = tempo_str.parse::<f64>().with_context(|| {
                                    format!("Error loading tempo mark in {measure_name}")
                                })?;
                                bpm_changes.push((next_chord_start, new_bpm));
                            }
                        }
                    }
                    // Extract boomwhacker notes from `note` elements
                    "note" => {
                        add_whack(
                            elem,
                            divs_per_beat,
                            &bpm_changes,
                            &mut next_chord_start,
                            &mut current_chord_start,
                            &mut current_chord_note_idx,
                            &mut whacks_loaded_so_far,
                            &mut whacks,
                        )
                        .ok_or_else(|| {
                            anyhow::Error::msg(format!("Error loading note in {measure_name}",))
                        })?;
                    }
                    _ => {}
                }
            }
        }
    }

    // Sort the whack times, and return
    for times in whacks.values_mut() {
        times.sort();
    }
    Ok(whacks)
}

// TODO: Wrap the context into a struct
#[must_use]
fn add_whack(
    elem: &elementtree::Element,
    divs_per_beat: usize,
    bpm_changes: &[(Timestamp, f64)],
    next_chord_start: &mut Timestamp,
    current_chord_start: &mut Timestamp,
    chord_note_idx: &mut usize,
    whacks_loaded_so_far: &mut usize,
    whacks: &mut HashMap<Note, Vec<Whack>>,
) -> Option<()> {
    // Check that multiple voicings aren't being used
    let voice = match elem.find("voice") {
        Some(voice_elem) => voice_elem.text().parse::<usize>().ok()?,
        None => 1, // If no voice tag is given, assign it to the first voice
    };
    assert_eq!(voice, 1, "Multiple voices aren't implemented yet");

    // If this is the first note/rest in a chord, compute the start time of the
    // next note to come after it
    if elem.find("chord").is_none() {
        let note_duration = note_duration(elem, divs_per_beat, bpm_changes, *next_chord_start)?;

        // We're starting a chord (which may have only one note), so mark that
        // the *next* note will come after this one
        *current_chord_start = *next_chord_start;
        *chord_note_idx = *whacks_loaded_so_far;
        next_chord_start.secs.0 += note_duration.as_secs_f64();
    }

    // Actually add the note
    match elem.find("pitch") {
        // If the 'note' has a pitch, work out which boomwhacker this note
        // actually plays and add its start
        Some(pitch_elem) => {
            let octave = pitch_elem.find("octave")?.text().parse::<i8>().ok()?;
            let note_name = pitch_elem.find("step")?.text();
            let alter = match pitch_elem.find("alter") {
                Some(alter_elem) => alter_elem.text().parse::<i8>().ok()?,
                None => 0,
            };
            let whack = Whack {
                timestamp: *current_chord_start,
                note_idx: *whacks_loaded_so_far,
                chord_note_idx: *chord_note_idx,
            };
            *whacks_loaded_so_far += 1;
            whacks
                .entry(Note::from_note(octave, note_name, alter)?)
                .or_default()
                .push(whack);
        }
        // If a 'note' has no pitch, it must be a rest
        None => assert!(elem.find("rest").is_some()),
    }
    Some(())
}

/// Load the number of divisions per beat, for a given part
fn divisions_per_beat(part_elem: &elementtree::Element) -> Option<usize> {
    part_elem
        .children()
        .next()?
        .find("attributes")?
        .find("divisions")?
        .text()
        .parse()
        .ok()
}

fn note_duration(
    elem: &elementtree::Element,
    divs_per_beat: usize,
    bpm_changes: &[(Timestamp, f64)],
    next_chord_start: Timestamp,
) -> Option<Duration> {
    let num_divs_in_note = elem.find("duration")?.text().parse::<u32>().ok()?;
    // Get the BPM at this note, so we know how long each `division` is
    let current_bpm_idx = bpm_changes
        .binary_search_by_key(&next_chord_start, |(dur, _new_bpm)| *dur)
        .map_or_else(|gap_idx| gap_idx.saturating_sub(1), |hit_idx| hit_idx);
    let current_bpm = bpm_changes
        .get(current_bpm_idx)
        .map_or(120.0, |(_start, bpm)| *bpm);
    let div_duration = Duration::from_secs_f64(60.0 / current_bpm / divs_per_beat as f64);
    let note_duration = div_duration * num_divs_in_note;
    Some(note_duration)
}

/// Indication of a point in time where a note starts
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    secs: OrderedFloat<f64>,
}

impl Timestamp {
    pub const ZERO: Self = Timestamp {
        secs: OrderedFloat(0.0),
    };

    pub const MAX: Self = Timestamp {
        secs: OrderedFloat(f64::MAX),
    };

    pub fn secs_until(self, other: Self) -> f64 {
        other.secs.0 - self.secs.0
    }
}

impl std::fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:>6.2}s", self.secs)
    }
}

///////////////////////////////
// CREATING ANNOTATED SCORES //
///////////////////////////////

impl MusicXmlScore {
    /// Returns MusicXML to describe this `MusicXmlScore`, with the notes of the given
    /// `{left,right}_hand`s annotated with lyric marks.
    pub fn annotated_xml(&self, left_hand: &[Note], right_hand: &[Note]) -> String {
        // We label notes sorted from highest to lowest (because, in MusicXML, lyric marks are
        // written from top to bottom, and we want the highest notes to be at the top).
        let mut notes = Vec::new();
        notes.extend(left_hand.iter().map(|note| (*note, Hand::Left)));
        notes.extend(right_hand.iter().map(|note| (*note, Hand::Right)));
        notes.sort_by_key(|(note, _)| Reverse(*note));
        // Decide which notes need to be coloured
        let mut coloured_notes = HashMap::<usize, Hand>::new();
        for &(note, hand) in &notes {
            for whack in &self.whacks[&note] {
                coloured_notes.insert(whack.note_idx, hand);
            }
        }
        // Decide which notes need `<lyric>` tags
        let mut lyric_locations = HashMap::<usize, Vec<(Note, Hand)>>::new();
        for &(note, hand) in &notes {
            for whack in &self.whacks[&note] {
                lyric_locations
                    .entry(whack.chord_note_idx)
                    .or_default()
                    .push((note, hand));
            }
        }
        // Traverse the XML tree, modifying it so that the only lyric marks are those of the notes
        // played by this player
        let mut new_tree = self.tree.clone();
        let mut note_idx = 0;
        for part in new_tree.find_all_mut("part") {
            for measure in part.children_mut() {
                for note_elem in measure.children_mut().filter(|c| c.tag().name() == "note") {
                    if note_elem.find("rest").is_some() {
                        assert!(note_elem.find("pitch").is_none());
                        continue; // Skip rests
                    }
                    // Colour the note
                    let colour = coloured_notes
                        .get(&note_idx)
                        .map_or("#000000", |hand| hand.colour());
                    note_elem.set_attr("color", colour);
                    // Remove any existing `<lyric>` tags
                    // TODO: Add `retain_children` to `elementtree`
                    let indices_of_lyrics = note_elem
                        .children()
                        .positions(|elem| elem.tag().name() == "lyric")
                        .collect_vec();
                    for idx in indices_of_lyrics.into_iter().rev() {
                        note_elem.remove_child(idx);
                    }
                    // Add our own lyric tags
                    for (note, hand) in lyric_locations.get(&note_idx).unwrap_or(&Vec::new()) {
                        let lyric_elem = note_elem
                            .append_new_child("lyric")
                            .set_attr("color", hand.colour())
                            .set_attr("number", "1");
                        lyric_elem.append_new_child("syllabic").set_text("single");
                        lyric_elem.append_new_child("text").set_text(note.name());
                    }
                    // Update the `note_idx` now that we've finished with this note
                    note_idx += 1;
                }
            }
        }
        // Return `new_tree` as an XML string
        new_tree.to_string().unwrap()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hand {
    Left,
    Right,
}

impl Hand {
    fn colour(self) -> &'static str {
        match self {
            Hand::Left => "#ff0000",
            Hand::Right => "#00aa00",
        }
    }
}
