//! Contains the model for user-provided file extensions

use std::error::Error;
use std::str::FromStr;
use core::fmt::{self, Display, Debug, Formatter};

pub struct FileExt(pub String);

pub const MAX_EXTENSION_LENGTH: usize = 32;

#[derive(Debug, Copy, Clone)]
pub enum ExtensionError {
    NotAlphanumeric(char),
    DoesNotStartWithDot(char),
    ConsecutiveDots,
    EndsWithDot,
    EmptyExtension,
    TooLong(usize)
}

impl Error for ExtensionError {}

impl Display for ExtensionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAlphanumeric(chr) => { write!(f, "file extension must be alphanumeric, but got non-alphanumeric '{chr}'") }
            Self::DoesNotStartWithDot(chr) => { write!(f, "expected first character of file extension to be a dot, but got '{chr}'") }
            Self::ConsecutiveDots => { write!(f, "file extension contains multiple dots in a row, which isn't allowed") }
            Self::EndsWithDot => { write!(f, "file extension must end with an alphanumeric character, not a dot") }
            Self::TooLong(len) => { write!(f, "file extension must be limited to {MAX_EXTENSION_LENGTH} characters, but got {len}") }
            Self::EmptyExtension => { write!(f, "file extension must not be specified as empty") }
        }
    }
}

impl FromStr for FileExt {
    type Err = ExtensionError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() > MAX_EXTENSION_LENGTH {
            return Err(Self::Err::TooLong(value.len()));
        }
        let chars = value.chars();

        enum State {
            WantDot, WantLetters, WantLettersOrDot
        }

        let mut state: State = State::WantDot;

        for chr in chars { match state {
            State::WantDot => {
                if chr != '.' {
                    return Err(Self::Err::DoesNotStartWithDot(chr));
                }

                state = State::WantLetters;
            }
            State::WantLetters => {
                if chr == '.' {
                    return Err(Self::Err::ConsecutiveDots);
                }

                if !chr.is_alphanumeric() {
                    return Err(Self::Err::NotAlphanumeric(chr));
                }

                state = State::WantLettersOrDot;
            }
            State::WantLettersOrDot => {
                if chr == '.' {
                    state = State::WantLetters;
                    continue;
                }

                if !chr.is_alphanumeric() {
                    return Err(Self::Err::NotAlphanumeric(chr));
                }

                continue
            }
        }}

        return match state {
            State::WantDot => Err(Self::Err::EmptyExtension),
            State::WantLetters => Err(Self::Err::EndsWithDot),
            State::WantLettersOrDot => Ok(FileExt(value.to_owned()))
        }
    }
}
