use std::{path::PathBuf, time::Instant};

use anyhow::Context;
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

    // Print the whack times
    for (whacker, times) in score.whacks.iter().sorted_by_key(|(w, _)| *w) {
        println!(
            "{:>3}: {:.2?}",
            whacker.name(),
            times.into_iter().map(|w| w.timestamp).collect_vec()
        );
    }
    println!("{} boomwhackers required", score.whacks.len());
    println!();

    let num_players = 7; // TODO: Make number of players no longer hard-coded

    // Start searching for good assignments (for seven players)
    let search_start = Instant::now();
    let assignment = Assignment::search(&score, num_players, 0);
    assignment.print();
    println!(
        "Found best score of {:.3} in {:.2?}",
        assignment.score,
        search_start.elapsed()
    );

    // Create temporary directories for working with files
    let temp_dir = PathBuf::from("./boomwhackers/");
    let music_xml_dir = temp_dir.join("music_xml");
    let pdf_dir = temp_dir.join("pdfs");
    std::fs::create_dir_all(&music_xml_dir).context("Couldn't create musicXML directory")?;
    std::fs::create_dir_all(&pdf_dir).context("Couldn't create pdf directory")?;
    // Construct musicXML files for each player
    for (idx, (left_hand, right_hand)) in assignment.players.iter().enumerate() {
        let music_xml_path = music_xml_dir.join(&format!("player-{idx}.musicxml"));
        let xml = score.annotated_xml(left_hand, right_hand);
        std::fs::write(&music_xml_path, xml.as_bytes())?;
    }
    // Create a JSON file with instructions for musescore's bulk conversion
    let mut conversion_jobs = Vec::new();
    let mut pdf_paths = Vec::new();
    for player_num in 0..num_players {
        let music_xml_path = music_xml_dir.join(&format!("player-{player_num}.musicxml"));
        let pdf_path = pdf_dir.join(&format!("player-{player_num}.pdf"));
        conversion_jobs.push(format!(
            r#"{{ "in": {music_xml_path:?}, "out": {pdf_path:?} }}"#
        ));
        pdf_paths.push(pdf_path);
    }
    let jobs_json = format!("[\n  {}\n]", conversion_jobs.iter().join(",\n  "));
    let musescore_job_path = temp_dir.join("convert.json");
    std::fs::write(&musescore_job_path, jobs_json.as_bytes())?;
    // Bulk-convert musicXML files to PDFs (i.e. create one PDF per player)
    std::process::Command::new("musescore3")
        .args(["-j", musescore_job_path.as_os_str().to_str().unwrap()])
        .spawn()?
        .wait()?;
    // Combine these PDFs into one large PDF
    std::process::Command::new("pdftk")
        .args(pdf_paths)
        .args(["cat", "output"])
        .args(["combined.pdf"])
        .spawn()?
        .wait()?;
    // Delete the temp working files
    std::fs::remove_dir_all(temp_dir)?;

    Ok(())
}
