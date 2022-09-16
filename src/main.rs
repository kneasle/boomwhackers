use std::path::PathBuf;

use itertools::Itertools;

use crate::music_xml::MusicXmlScore;

mod assignment;
mod note;
mod search;

mod assign;
mod music_xml;

fn main() -> anyhow::Result<()> {
    // Get the input file path
    let input_file_path: PathBuf = std::env::args()
        .nth(1)
        .expect("Expected first arg to be the file-name")
        .into();
    // Load the MusicXML file and extract the whacks
    let score = MusicXmlScore::load_file(input_file_path)?;
    for (whacker, times) in score.whacks.iter().sorted_by_key(|(w, _)| *w) {
        println!("{:>3}: {:.2?}", whacker.name(), times);
    }
    println!("{} boomwhackers required", score.whacks.len());
    println!();

    // Start searching for good assignments (for 14 hands)
    // TODO: Make number of hands no longer hard-coded
    let result = crate::search::search(14, &score.whacks);
    let max_num_whackers_in_left_hand = result
        .hand_assignment
        .iter()
        .step_by(2)
        .map(|l| l.len())
        .max()
        .unwrap();
    for (left, right) in result.hand_assignment.iter().tuples() {
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

    Ok(())
}
