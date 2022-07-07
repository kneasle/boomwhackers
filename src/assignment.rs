use std::{ops::Range, time::Duration};

use itertools::Itertools;
use rand::{prelude::SliceRandom, Rng};

use crate::{
    search::{Context, WhackerIdx},
    whacker::Whacker,
};

/// An assignment of [`Whacker`]s to hands.  We don't worry about combining the hands yet; that's
/// up to the next stage of the search.
#[derive(Debug, Clone)]
pub struct HandAssignment {
    /// A flat list representing all the [`Hand`]s' [`Whacker`] assignments concatenated together.
    ///
    /// Storing them as a single flat list makes [`Self::make_swap`] substantially easier and more
    /// efficient (since we can uniformly sample two whackers from this list).  Also we use
    /// [`WhackerIdx`]s instead of [`Whacker`]s for more efficient lookups.
    whackers: Vec<WhackerIdx>,
    /// Each [`Hand`] is assigned to some sub-[`Range`] of `whackers`
    hands: Vec<Hand>,
    /// The current score of the `Assignment`
    score: f64,
}

#[derive(Debug, Clone)]
struct Hand {
    /// Score generated by this hand's swaps
    // score: f64,
    /// The [`Range`] of the slice of [`Whacker`]s which are played by this hand
    range: Range<usize>,
}

// /// A representation storing enough information to undo a `Swap` of two boomwhackers in an
// /// [`Assignment`].
// #[derive(Debug)]
// pub struct Swap {
//     old_score: f64,
//     swap_idx_1: usize,
//     swap_idx_2: usize,
// }

impl HandAssignment {
    /// Create a new `Assignment` where all the [`Whacker`]s are randomly assigned.
    pub fn random(num_hands: usize, ctx: &Context, rng: &mut impl Rng) -> Self {
        // Shuffle the `WhackerIdx`s to create the random starting assignment
        let mut whackers = ctx
            .whacks
            .iter_enumerated()
            .map(|(idx, _)| idx)
            .collect_vec();
        whackers.shuffle(rng);

        // Split the assignment up into hands, as evenly as possible.  I.e. we assign the same
        // number of whackers to all the hands, with some hands taking one extra.
        let base_whackers_per_hand = whackers.len() / num_hands;
        let num_hands_with_one_extra = whackers.len() % num_hands;

        let mut hands = Vec::new();
        let mut whackers_allocated = 0;
        for i in 0..num_hands {
            // Give the spare whackers to the first `num_hands_with_one_extra` hands
            let num_whackers =
                base_whackers_per_hand + if i < num_hands_with_one_extra { 1 } else { 0 };
            hands.push(Hand::new(
                &whackers,
                whackers_allocated..whackers_allocated + num_whackers,
                ctx,
            ));
            whackers_allocated += num_whackers;
        }

        Self {
            score: hands.iter().map(|hand| hand.score(&whackers, ctx)).sum(),
            hands,
            whackers,
        }
    }

    /// Swap two boomwhackers in this `Assignment`, returning a [`Swap`] object that can be used to
    /// undo the swap if needed.
    pub fn make_swap(&mut self, ctx: &Context, rng: &mut impl Rng) {
        let swap_idx_1 = rng.gen_range(0..self.whackers.len());
        let swap_idx_2 = rng.gen_range(0..self.whackers.len());

        self.whackers.swap(swap_idx_1, swap_idx_2);
        self.score = self
            .hands
            .iter()
            .map(|hand| hand.score(&self.whackers, ctx))
            .sum();
    }

    pub fn score(&self) -> f64 {
        self.score
    }

    pub fn whackers(&self, ctx: &Context) -> Vec<Vec<Whacker>> {
        self.hands
            .iter()
            .map(|hand| {
                self.whackers[hand.range.clone()]
                    .iter()
                    .map(|idx| ctx.whacks[*idx].0)
                    .collect_vec()
            })
            .collect_vec()
    }
}

impl Hand {
    fn new(whackers: &[WhackerIdx], range: Range<usize>, ctx: &Context) -> Self {
        Self {
            // score: score_for_hand(&whackers[range.clone()], ctx),
            range,
        }
    }

    fn score(&self, whackers: &[WhackerIdx], ctx: &Context) -> f64 {
        score_for_hand(&whackers[self.range.clone()], ctx)
    }
}

/// Given a set of [`Whacker`]s which need to be played by a single hand, compute the score
/// generated from the swaps.  All swaps contribute negative score, and this score is weighted
/// by how long the swap requires.
fn score_for_hand(whackers_in_hand: &[WhackerIdx], ctx: &Context) -> f64 {
    if whackers_in_hand.len() <= 1 {
        return 0.0; // Any hand with 0 or 1 whackers doesn't need any swaps
    }

    let mut score = 0.0;

    // If there are at least two whackers that have to be played by this hand, then we need to
    // detect how long the player has to swap them.  Since the `Duration` vectors are sorted,
    // we can detect swaps by merging the lists of times (like in merge sort) and keeping track
    // of how many times we have to switch.

    let mut whack_time_iterators = whackers_in_hand
        .iter()
        .map(|whacker_idx| ctx.whacks[*whacker_idx].1.iter().peekable())
        .collect_vec();

    // Find the whacker with the first time, and assume the player starts holding that whacker
    let mut last_played_iter_idx = whackers_in_hand
        .iter()
        .position_min_by_key(|idx| ctx.whacks[**idx].1[0])
        .unwrap(); // Can't panic because early return guarantees >1 whacker
    let mut last_whack_time = Duration::ZERO;
    loop {
        // Determine which boomwhacker is the next to play
        let mut best_next_time = Duration::MAX;
        let mut next_iter_idx = None;
        for (iter_idx, times) in whack_time_iterators.iter_mut().enumerate() {
            if let Some(&&next_time) = times.peek() {
                if next_time < best_next_time {
                    best_next_time = next_time;
                    next_iter_idx = Some(iter_idx);
                }
            }
        }
        let next_iter_idx = match next_iter_idx {
            Some(idx) => idx,
            None => break, // All iters have finished, so all the notes have been played
        };

        // Consume the next hit from the corresponding iterator
        assert_eq!(
            whack_time_iterators[next_iter_idx].next(),
            Some(&best_next_time)
        );

        // Update score if this hit requires us to switch boomwhackers
        if last_played_iter_idx != next_iter_idx {
            let mut time_diff = (best_next_time - last_whack_time).as_secs_f64();
            if time_diff < 0.01 {
                time_diff = 0.01;
            }
            // For swapping boomwhackers, the score should be roughly reciprocal in the time -
            // i.e. getting really close gets bad very quickly, but the differences becomes
            // much less relevant once we have a few seconds for the switch.
            score -= 1.0 / time_diff;
        }
        last_whack_time = best_next_time;
        last_played_iter_idx = next_iter_idx;
    }

    score
}
