use std::{path::PathBuf, time::Instant};

use itertools::Itertools;

use crate::{assign::Assignment, music_xml::MusicXmlScore};

mod assign;
mod music_xml;
mod note;

fn main() -> anyhow::Result<()> {
    // Get the input file path
    let input_file_path: PathBuf = std::env::args()
        .nth(1)
        .expect("Expected first arg to be the file-name")
        .into();
    // Load the MusicXML file and extract the whacks
    let score = MusicXmlScore::load_file(input_file_path)?;
    for (whacker, times) in score.whacks.iter().sorted_by_key(|(w, _)| *w) {
        println!(
            "{:>3}: {:.2?}",
            whacker.name(),
            times.into_iter().map(|w| w.timestamp).collect_vec()
        );
    }
    println!("{} boomwhackers required", score.whacks.len());
    println!();

    // Start searching for good assignments (for seven players)
    let search_start = Instant::now();
    // TODO: Make number of players no longer hard-coded
    let assignment = Assignment::new(&score, 7, 0);
    let max_num_whackers_in_left_hand = assignment
        .players
        .iter()
        .map(|(left, _right)| left.len())
        .max()
        .unwrap();
    for (left, right) in &assignment.players {
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
        assignment.score,
        search_start.elapsed()
    );

    Ok(())
}
