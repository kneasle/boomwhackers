//! Code for computing the assignment of boomwhackers to players

use std::ops::Range;

use itertools::Itertools;
use ordered_float::OrderedFloat;
use rand::{seq::SliceRandom, Rng, SeedableRng};

use crate::{
    music_xml::{MusicXmlScore, Timestamp},
    note::Note,
};

/// An `Assignment` of boomwhackers to players.
#[derive(Debug, Clone)]
pub struct Assignment {
    pub players: Vec<(Vec<Note>, Vec<Note>)>,
    pub score: f64,
}

impl Assignment {
    pub fn new(music: &MusicXmlScore, num_players: usize, seed: u64) -> Self {
        let fast_assignment = FastAssignment::from_search(music, num_players, seed);
        Self {
            score: fast_assignment.score(music),
            players: fast_assignment
                .players
                .into_iter()
                .map(|(left_range, right_range)| {
                    (
                        fast_assignment.whackers[left_range].to_vec(),
                        fast_assignment.whackers[right_range].to_vec(),
                    )
                })
                .collect_vec(),
        }
    }
}

////////////
// SEARCH //
////////////

/// An `Assignment` of boomwhackers to players, optimised for the operations used by the search.
#[derive(Debug, Clone)]
struct FastAssignment {
    /// A flat list representing all the [`Hand`]s' [`Whacker`] assignments concatenated together.
    ///
    /// Storing them as a single flat list makes [`Self::make_swap`] substantially easier and more
    /// efficient (since we can uniformly sample two whackers from this list).  Also we use
    /// [`WhackerIdx`]s instead of [`Whacker`]s for more efficient lookups.
    whackers: Vec<Note>,
    /// Each [`Hand`] is assigned to some sub-[`Range`] of `whackers`
    players: Vec<(Range<usize>, Range<usize>)>,
}

impl FastAssignment {
    /// Search for an `Assignment` which works well for the given [`MusicXmlScore`].
    fn from_search(music: &MusicXmlScore, num_players: usize, seed: u64) -> Self {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
        // Run 100 runs of `gradient_ascent`, each starting from a random assignment
        let mut assignment = (0..100)
            .map(|_| Self::gradient_ascent(music, num_players, &mut rng))
            .max_by_key(|assignment| OrderedFloat(assignment.score(&music)))
            .unwrap();
        // Sort the hands by their lowest `Note`, and re-pair them.  TODO: Assign hand patterns
        // during search
        let mut hands = std::mem::take(&mut assignment.players)
            .into_iter()
            .flat_map(|(l, r)| [l, r])
            .collect_vec();
        for hand_range in &hands {
            assignment.whackers[hand_range.clone()].sort(); // Sort individual ranges
        }
        hands.sort_by_key(|range| assignment.whackers[range.clone()].first().copied());
        assignment.players = hands.into_iter().tuples().collect_vec();

        assignment
    }

    /// Perform one run of stochastic gradient 'ascent' to generate one pretty-well-optimised
    /// [`HandAssignment`]
    fn gradient_ascent(
        music: &MusicXmlScore,
        num_players: usize,
        rng: &mut impl Rng,
    ) -> FastAssignment {
        let mut assignment = FastAssignment::random(music, num_players, rng);
        let mut next_assignment = assignment.clone();
        for _ in 0..1_000 {
            // Try to generate another assignment by swapping some values
            next_assignment.clone_from(&assignment);
            next_assignment.make_swap(rng);
            // If the new assignment is better, move to it
            if next_assignment.score(&music) > assignment.score(&music) {
                std::mem::swap(&mut assignment, &mut next_assignment);
            }
        }
        assignment
    }

    /// Create a new `Assignment` where all the [`Whacker`]s are randomly assigned.
    fn random(music: &MusicXmlScore, num_players: usize, rng: &mut impl Rng) -> Self {
        let num_hands = num_players * 2;
        // Shuffle the `WhackerIdx`s to create the random starting assignment
        let mut whackers = music.whacks.keys().copied().collect_vec();
        whackers.sort(); // Makes search deterministic despite nondeterminism of `HashMap::keys()`
        whackers.shuffle(rng);
        // Determine how many whackers must be given to each hand (with a few hands taking one
        // extra to make the difference).  I.e. we assign the same number of whackers to all the
        // hands, with some hands taking one extra.
        let base_whackers_per_hand = whackers.len() / num_hands;
        let num_hands_with_one_extra = whackers.len() % num_hands;
        // Determine what ranges are given to each hand
        let mut hands = Vec::<Range<usize>>::new();
        let mut whackers_allocated = 0;
        for i in 0..num_hands {
            // Give the spare whackers to the first `num_hands_with_one_extra` hands
            let num_whackers =
                base_whackers_per_hand + if i < num_hands_with_one_extra { 1 } else { 0 };
            hands.push(whackers_allocated..whackers_allocated + num_whackers);
            whackers_allocated += num_whackers;
        }
        // Group the hands into players
        assert_eq!(hands.len() % 2, 0);
        let players: Vec<(_, _)> = hands.into_iter().tuples().collect_vec();

        Self { players, whackers }
    }

    /// Swap two boomwhackers in this `Assignment`, returning a [`Swap`] object that can be used to
    /// undo the swap if needed.
    fn make_swap(&mut self, rng: &mut impl Rng) {
        let swap_idx_1 = rng.gen_range(0..self.whackers.len());
        let swap_idx_2 = rng.gen_range(0..self.whackers.len());
        self.whackers.swap(swap_idx_1, swap_idx_2);
    }

    // TODO/PERF: Cache scores (and possibly also intermediate values)
    fn score(&self, music: &MusicXmlScore) -> f64 {
        let mut score = 0.0;
        for (left_range, right_range) in &self.players {
            score += score_for_player(
                &self.whackers[left_range.clone()],
                &self.whackers[right_range.clone()],
                music,
            );
        }
        score
    }
}

/// Given the [`Note`]s of the whackers played by each hand of a player, compute the score
/// generated from that player having to swap which whacker they hold in each hand.  All swaps
/// contribute negative score, and this score is weighted by (the inverse of) how long the swap
/// requires.
fn score_for_player(left_hand: &[Note], right_hand: &[Note], music: &MusicXmlScore) -> f64 {
    score_for_hand(left_hand, music) + score_for_hand(right_hand, music)
}

/// Given a set of [`Whacker`]s which need to be played by a single hand, compute the score
/// generated from the swaps.  All swaps contribute negative score, and this score is weighted
/// by how long the swap requires.
fn score_for_hand(whackers_in_hand: &[Note], music: &MusicXmlScore) -> f64 {
    if whackers_in_hand.len() <= 1 {
        return 0.0; // Any hand with 0 or 1 whackers doesn't need any swaps
    }

    let mut score = 0.0;

    // If there are at least two whackers that have to be played by this hand, then we need to
    // detect how long the player has to swap them.  Since the `Duration` vectors are sorted, we
    // can detect swaps by merging the lists of times (like in merge sort) and counting how many
    // times we had to switch between those lists.

    let mut whack_iterators = whackers_in_hand
        .iter()
        .map(|note| music.whacks[note].iter().peekable())
        .collect_vec();

    // Find the whacker with the first time, and assume the player starts holding that whacker
    let mut last_played_iter_idx = whackers_in_hand
        .iter()
        .position_min_by_key(|idx| music.whacks[*idx][0])
        .unwrap(); // Can't panic because early return guarantees >1 whacker
    let mut last_whack_time = Timestamp::ZERO;
    loop {
        // Determine which boomwhacker is the next to play
        let mut best_next_time = Timestamp::MAX;
        let mut next_iter_idx = None;
        for (iter_idx, whack) in whack_iterators.iter_mut().enumerate() {
            if let Some(next_whack) = whack.peek() {
                let next_time = next_whack.timestamp;
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
            whack_iterators[next_iter_idx].next().map(|w| w.timestamp),
            Some(best_next_time)
        );

        // Update score if this hit requires us to switch boomwhackers
        if last_played_iter_idx != next_iter_idx {
            let mut time_diff = last_whack_time.secs_until(best_next_time);
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
