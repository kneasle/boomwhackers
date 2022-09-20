use std::fmt::{Debug, Display, Formatter};

/// Representation of the note to which a single boomwhacker is tuned
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Note {
    /// Semitones above C0
    pub semis_above_c0: i8,
}

impl Note {
    pub fn from_note(octave: i8, note_name: &str, alter: i8) -> Option<Self> {
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

    pub fn name(&self) -> String {
        // Split `self.semis_above_c0` into `(octave * 12) + semis_above_nearest_c`
        let semis_above_nearest_c = self.semis_above_c0.rem_euclid(12);
        let octave = self.semis_above_c0.div_euclid(12);

        let note_name = NOTE_NAMES_SHARPS[semis_above_nearest_c as usize];
        format!("{note_name}{octave}")
    }

    pub fn name_flats(&self) -> String {
        // Split `self.semis_above_c0` into `(octave * 12) + semis_above_nearest_c`
        let semis_above_nearest_c = self.semis_above_c0.rem_euclid(12);
        let octave = self.semis_above_c0.div_euclid(12);

        let note_name = NOTE_NAMES_FLATS[semis_above_nearest_c as usize];
        format!("{note_name}{octave}")
    }
}

impl Display for Note {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:>3}", self.name())
    }
}

impl Debug for Note {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Whacker({})", self)
    }
}

const NOTE_NAMES_SHARPS: [&str; 12] = [
    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
];
const NOTE_NAMES_FLATS: [&str; 12] = [
    "C", "D♭", "D", "E♭", "E", "F", "G♭", "G", "A♭", "A", "B♭", "B",
];
