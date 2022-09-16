use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use itertools::Itertools;
use ordered_float::OrderedFloat;
use rand::prelude::*;

use crate::{assignment::HandAssignment, music_xml::Timestamp, note::Note};

#[derive(Debug)]
pub struct SearchResult {
    // pub whackers: Vec<(Vec<Whacker>, Vec<Whacker>)>,
    pub hand_assignment: Vec<Vec<Note>>,
    pub best_score: f64,
    pub duration: Duration,
}

pub fn search(num_hands: usize, whacks: &HashMap<Note, Vec<Timestamp>>) -> SearchResult {
    let start = Instant::now();

    // Allow `Whacker`s to be referenced by numerical `WhackerIdx`s, which are assigned
    // contiguously from 0.
    let ctx = Context {
        whacks: whacks
            .iter()
            .sorted_by_key(|(w, _hits)| *w) // Sort the whackers so that the search is deterministic
            .map(|(w, hits)| (*w, hits.clone()))
            .collect(),
    };

    // Run 100 runs of `gradient_ascent`, each starting from a random assignment
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
    let hand_assignment = (0..100)
        .map(|_| gradient_ascent(num_hands, &ctx, &mut rng))
        .max_by_key(|a| OrderedFloat(a.score()))
        .unwrap();

    // Extract the whacker assignment and sort everything
    let mut whacker_assignment = hand_assignment.whackers(&ctx);
    for hand in &mut whacker_assignment {
        hand.sort();
    }
    whacker_assignment.sort_by_key(|whackers_in_hand| whackers_in_hand.first().copied());

    SearchResult {
        hand_assignment: whacker_assignment,
        best_score: hand_assignment.score(),
        duration: start.elapsed(),
    }
}

/// Perform one run of stochastic gradient 'ascent' to generate one pretty-well-optimised
/// [`Assignment`]
fn gradient_ascent(
    num_hands: usize,
    ctx: &Context,
    rng: &mut rand_chacha::ChaCha8Rng,
) -> HandAssignment {
    let mut assignment = HandAssignment::random(num_hands, ctx, rng);
    let mut next_assignment = assignment.clone();
    for _ in 0..1_000 {
        // Try to generate another assignment by swapping some values
        next_assignment.clone_from(&assignment);
        next_assignment.make_swap(ctx, rng);
        // If the new assignment is better, move to it
        if next_assignment.score() > assignment.score() {
            std::mem::swap(&mut assignment, &mut next_assignment);
        }
    }
    assignment
}

/// Immutable context for a search
#[derive(Debug)]
pub struct Context {
    pub whacks: WhackerVec<(Note, Vec<Timestamp>)>,
}

index_vec::define_index_type! { pub struct WhackerIdx = u8; }
pub type WhackerVec<T> = index_vec::IndexVec<WhackerIdx, T>;
