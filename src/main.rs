use std::{fs::File, io::Read, path::PathBuf};

use itertools::Itertools;

mod load;

fn main() {
    // Get the input file path
    let input_file_path: PathBuf = std::env::args()
        .skip(1)
        .next()
        .expect("Expected first arg to be the file-name")
        .into();
    // Load the XML file as a tree
    let xml_bytes = read_xml(input_file_path);
    let tree = elementtree::Element::from_reader(xml_bytes.as_slice()).unwrap();

    // Extract and print the boomwhacker timings
    let whacks = crate::load::load_whacks(tree).expect("Failed to load whacks");
    for (whacker, times) in whacks.iter().sorted_by_key(|(w, _)| *w) {
        println!("{:>3}: {:.2?}", whacker.name(), times);
    }
    println!("{} boomwhackers required", whacks.len());
}

/// Read a music XML file, possibly decompressing it if necessary
fn read_xml(input_file_path: PathBuf) -> Vec<u8> {
    let mut file = File::open(&input_file_path).expect("Error loading file");
    let mut xml_bytes = Vec::new();
    match input_file_path.extension().unwrap().to_str() {
        Some("xml") => file
            .read_to_end(&mut xml_bytes)
            .expect("Expected utf-8 file"),
        Some("mxl") => {
            let mut unzipped = zip::ZipArchive::new(&file).expect("Failed to load zip file");
            let xml_file_name = unzipped
                .file_names()
                .find(|f| !f.contains('/'))
                .expect("Should have at least one file in the zip archive")
                .to_owned();
            let mut xml_file = unzipped.by_name(&xml_file_name).unwrap();
            xml_file
                .read_to_end(&mut xml_bytes)
                .expect("Error whilst reading string")
        }
        Some(s) => panic!("Unknown file extension {s:?}.  Expected `.xml` or `.mxl`"),
        None => panic!("Can't open file without an extension"),
    };
    xml_bytes
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
struct Whacker {
    /// Semitones above C0
    semis_above_c0: i8,
}

impl Whacker {
    fn from_note(octave: i8, note_name: &str, alter: i8) -> Option<Self> {
        let note_semitones_from_c = match note_name {
            "C" => 0i8,
            "D" => 2,
            "E" => 4,
            "F" => 5,
            "G" => 7,
            "A" => 9,
            "B" => 11,
            _ => return None, // Invalid note name
        };
        Some(Self {
            semis_above_c0: octave * 12 + note_semitones_from_c + alter,
        })
    }

    fn name(&self) -> String {
        // Split `self.semis_above_c0` into `(octave * 12) + semis_above_nearest_c`
        let semis_above_nearest_c = self.semis_above_c0.rem_euclid(12);
        let octave = self.semis_above_c0.div_euclid(12);

        let note_name = NOTE_NAMES_SHARPS[semis_above_nearest_c as usize];
        format!("{note_name}{octave}")
    }

    #[allow(dead_code)]
    fn name_flats(&self) -> String {
        // Split `self.semis_above_c0` into `(octave * 12) + semis_above_nearest_c`
        let semis_above_nearest_c = self.semis_above_c0.rem_euclid(12);
        let octave = self.semis_above_c0.div_euclid(12);

        let note_name = NOTE_NAMES_FLATS[semis_above_nearest_c as usize];
        format!("{note_name}{octave}")
    }
}

const NOTE_NAMES_SHARPS: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const NOTE_NAMES_FLATS: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];
