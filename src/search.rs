use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use itertools::Itertools;
use ordered_float::OrderedFloat;
use rand::prelude::*;

use crate::{assignment::Assignment, whacker::Whacker};

#[derive(Debug)]
pub struct SearchResult {
    pub whackers: Vec<(Vec<Whacker>, Vec<Whacker>)>,
    pub best_score: f64,
    pub duration: Duration,
}

pub fn search(whacks: &HashMap<Whacker, Vec<Duration>>) -> SearchResult {
    // Allow `Whacker`s to be referenced by numerical `WhackerIdx`s, which are assigned
    // contiguously from 0.
    let ctx = Context {
        whacks: whacks
            .iter()
            .sorted_by_key(|(w, _hits)| *w) // Sort the whackers so that the search is deterministic
            .map(|(w, hits)| (*w, hits.clone()))
            .collect(),
    };

    let start = Instant::now();

    // Run 100 runs of `gradient_ascent`, each starting from a random assignment
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);
    let best_assignment = (0..100)
        .map(|_| gradient_ascent(&ctx, &mut rng))
        .max_by_key(|a| OrderedFloat(a.score()))
        .unwrap();

    // Extract the whacker assignment and sort everything
    let mut whacker_assignment = best_assignment.whackers(&ctx);
    for (left, right) in &mut whacker_assignment {
        left.sort();
        right.sort();
        if left.first() > right.first() {
            std::mem::swap(left, right);
        }
    }
    whacker_assignment.sort_by_key(|(l, r)| Option::min(l.first(), r.first()).copied());

    SearchResult {
        whackers: whacker_assignment,
        best_score: best_assignment.score(),
        duration: start.elapsed(),
    }
}

/// Perform one run of stochastic gradient 'ascent' to generate one pretty-well-optimised
/// [`Assignment`]
fn gradient_ascent(ctx: &Context, rng: &mut rand_chacha::ChaCha8Rng) -> Assignment {
    let mut assignment = Assignment::random(ctx, rng);
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
    pub whacks: WhackerVec<(Whacker, Vec<Duration>)>,
}

index_vec::define_index_type! { pub struct WhackerIdx = u8; }
pub type WhackerVec<T> = index_vec::IndexVec<WhackerIdx, T>;
