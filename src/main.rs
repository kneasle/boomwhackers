use std::{fs::File, io::Read, path::PathBuf};

use itertools::Itertools;

use crate::search::search;

mod assignment;
mod load;
mod search;
mod whacker;

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

    println!();

    // Start searching for good assignments (for 14 hands)
    // TODO: Make number of hands no longer hard-coded
    let result = search(14, &whacks);
    let max_num_whackers_in_left_hand = result
        .whackers
        .iter()
        .step_by(2)
        .map(|l| l.len())
        .max()
        .unwrap();
    for (left, right) in result.whackers.iter().tuples() {
        for _ in 0..(max_num_whackers_in_left_hand - left.len()) {
            print!("     ");
        }
        for w in left {
            print!("{:>3}  ", w.to_string());
        }
        print!("|");
        for w in right {
            print!("  {:>3}", w.to_string());
        }
        println!();
    }

    println!();

    println!(
        "Found best score of {:.3} in {:.2?}",
        result.best_score, result.duration
    );
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
