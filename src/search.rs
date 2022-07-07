use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use itertools::Itertools;
use rand::prelude::*;

use crate::{assignment::Assignment, whacker::Whacker};

pub fn search(whacks: &HashMap<Whacker, Vec<Duration>>) -> Assignment {
    // Allow `Whacker`s to be referenced by numerical `WhackerIdx`s, which are assigned
    // contiguously from 0.
    let whacks: WhackerVec<_> = whacks
        .iter()
        .sorted_by_key(|(w, _hits)| *w) // Sort the whackers so that the search is deterministic
        .map(|(w, hits)| (*w, hits.clone()))
        .collect();
    let ctx = Context { whacks };

    // Create an initial random `Assignment`
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);

    let start = Instant::now();
    let mut assignment = Assignment::random(&ctx, &mut rng);
    let mut next_assignment = assignment.clone();
    for _ in 0..100_000 {
        // Try to generate another assignment by swapping some values
        next_assignment.clone_from(&assignment);
        next_assignment.make_swap(&ctx, &mut rng);
        // If the new assignment is better, move to it
        if next_assignment.score() > assignment.score() {
            std::mem::swap(&mut assignment, &mut next_assignment);
        }
    }
    println!(
        "{:>7.3?} in {:>9.2?} :: {:?}",
        assignment.score(),
        start.elapsed(),
        assignment.whackers
    );

    todo!()
}

/// Immutable context for a search
#[derive(Debug)]
pub struct Context {
    pub whacks: WhackerVec<(Whacker, Vec<Duration>)>,
}

index_vec::define_index_type! { pub struct WhackerIdx = u8; }
pub type WhackerVec<T> = index_vec::IndexVec<WhackerIdx, T>;
