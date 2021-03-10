use std::default::Default;
use std::ffi::{OsStr, OsString};
use std::mem;
use std::str;
use std::vec;

use thiserror::Error;

// Always include both oschars_unix and oschars_windows so they both get
// type checked as much as possible.
// Then pick the appropriate implementation.
mod oschars_unix;
mod oschars_windows;
#[cfg(unix)]
use oschars_unix::split_valid;
#[cfg(windows)]
use oschars_windows::split_valid;

pub struct ArgWalker {
    next_args: vec::IntoIter<OsString>,
    state: State,
    hold_os: OsString,
    hold: String,
    flag_yielded: Option<String>,
}

pub enum Item<'a> {
    Flag(&'a str),
    Word(&'a str),
    End,
}
pub enum ItemOs<'a> {
    Flag(&'a str),
    Word(&'a OsStr),
    End,
}

impl<'a> ItemOs<'a> {
    pub fn into_unicode(self) -> Result<Item<'a>, ArgError> {
        match self {
            ItemOs::Flag(f) => Ok(Item::Flag(f)),
            ItemOs::Word(w) => match w.to_str() {
                Some(s) => Ok(Item::Word(s)),
                None => Err(ArgError::InvalidUnicode(OsString::from(w))),
            },
            ItemOs::End => Ok(Item::End),
        }
    }
}

#[derive(Debug, Clone, Error)]
pub enum ArgError {
    #[error("invalid unicode in argument {0:?}")]
    InvalidUnicode(OsString),
    #[error("unexpected parameter for flag {0}")]
    UnexpectedParameter(String),
    #[error("parameter missing for flag {0}")]
    ParameterMissing(String),
}

enum State {
    Long {
        flag: String,
        parm: Option<OsString>,
    },
    LongParm {
        flag: String,
        parm: OsString,
    },
    Shorts {
        letters: String,
        flag: String, // first letter of letters, with '-' prepended
        tail: OsString,
    },
    NotOption(OsString),
    Finished,
    Failed(ArgError),
}
use State::*;

impl ArgWalker {
    pub fn new<S, T>(args: T) -> Self
    where
        T: IntoIterator<Item = S>,
        OsString: From<S>,
    {
        let arg_vec: Vec<OsString> = args.into_iter().map(OsString::from).collect();
        let mut next_args = arg_vec.into_iter();
        let state = Self::state_from_arg(next_args.next());
        let hold_os = Default::default();
        let hold = Default::default();
        let flag_yielded = None;
        ArgWalker {
            next_args,
            state,
            hold_os,
            hold,
            flag_yielded,
        }
    }

    fn process_next_arg(&mut self) -> State {
        Self::state_from_arg(self.next_args.next())
    }

    fn state_from_arg(os_arg_opt: Option<OsString>) -> State {
        let os_arg = match os_arg_opt {
            None => {
                return Finished;
            }
            Some(s) => s,
        };
        let (head, tail) = split_valid(&os_arg);

        if os_arg == "-" {
            NotOption(os_arg)
        } else if head.starts_with("--") {
            if let Some(idx) = head.find("=") {
                // long option with argument
                let flag = head[..idx].to_string();
                let mut parm = OsString::from(&head[idx + 1..]);
                parm.push(tail);
                Long {
                    flag,
                    parm: Some(parm),
                }
            } else {
                // long option without argument, must be valid unicode
                if tail.is_empty() {
                    Long {
                        flag: head.to_string(),
                        parm: None,
                    }
                } else {
                    Failed(ArgError::InvalidUnicode(os_arg))
                }
            }
        } else if head.starts_with("-") {
            state_shorts(&head[1..], OsString::from(tail)).unwrap()
        } else {
            // non-flag argument
            NotOption(os_arg)
        }
    }

    pub fn peek_item(&self) -> Result<Item<'_>, ArgError> {
        self.peek_item_os().and_then(ItemOs::into_unicode)
    }

    pub fn peek_item_os(&self) -> Result<ItemOs<'_>, ArgError> {
        use ItemOs::*;

        match &self.state {
            Long { flag, .. } => Ok(Flag(&flag)),
            LongParm { flag, .. } => Err(ArgError::UnexpectedParameter(flag.to_string())),
            Shorts { flag, .. } => Ok(Flag(&flag)),
            NotOption(w) => Ok(Word(&w)),
            Finished => Ok(End),
            Failed(e) => Err(e.clone()),
        }
    }

    pub fn take_item(&mut self) -> Result<Item<'_>, ArgError> {
        self.take_item_os().and_then(ItemOs::into_unicode)
    }

    pub fn take_item_os(&mut self) -> Result<ItemOs<'_>, ArgError> {
        use ItemOs::*;
        use State::*;

        let mut state = State::Finished;
        mem::swap(&mut self.state, &mut state);
        let (new_state, result) = match state {
            Long {
                flag,
                parm: Some(p),
            } => {
                self.hold = flag.clone();
                (LongParm { flag, parm: p }, Ok(Flag(&self.hold)))
            }

            Long { flag, parm: None } => {
                self.hold = flag.clone();
                (self.process_next_arg(), Ok(Flag(&self.hold)))
            }

            LongParm { flag, .. } => {
                let err = ArgError::UnexpectedParameter(flag);
                (Failed(err.clone()), Err(err))
            }

            Shorts {
                flag,
                letters,
                tail,
            } => {
                // flag is "-X" where X is the first letter of letters.
                let other_letters = &letters[flag.len() - 1..];
                let st =
                    state_shorts(other_letters, tail).unwrap_or_else(|| self.process_next_arg());
                self.hold = flag;
                (st, Ok(Flag(&self.hold)))
            }

            NotOption(w) => {
                self.hold_os = w;
                (self.process_next_arg(), Ok(Word(&self.hold_os as &OsStr)))
            }

            Finished => (Finished, Ok(End)),

            Failed(e) => (Failed(e.clone()), Err(e)),
        };
        self.state = new_state;

        match result {
            Ok(Flag(f)) => self.flag_yielded = Some(f.to_string()),
            Ok(_) => self.flag_yielded = None,
            _ => {}
        }

        result
    }

    pub fn has_parameter(&self, free_standing: bool) -> bool {
        match &self.state {
            LongParm { .. } => true,
            Shorts { letters, tail, .. } => !(letters.is_empty() && tail.is_empty()),
            NotOption(_) => free_standing,
            _ => false,
        }
    }

    pub fn parameter(&mut self, free_standing: bool) -> Result<Option<String>, ArgError> {
        match self.parameter_os(free_standing) {
            Ok(None) => Ok(None),
            Ok(Some(w)) => match w.into_string() {
                Ok(s) => Ok(Some(s)),
                Err(w) => Err(ArgError::InvalidUnicode(w)),
            },
            Err(e) => Err(e),
        }
    }

    pub fn parameter_os(&mut self, free_standing: bool) -> Result<Option<OsString>, ArgError> {
        let mut old_state = Finished; // dummy
        mem::swap(&mut old_state, &mut self.state);
        let (new_state, result) = match old_state {
            Failed(err) => (Failed(err.clone()), Err(err)),

            LongParm { parm, .. } => (self.process_next_arg(), Ok(Some(parm))),

            Shorts { letters, tail, .. } => {
                let mut parm = OsString::from(letters);
                parm.push(tail);
                (self.process_next_arg(), Ok(Some(parm)))
            }

            NotOption(w) if free_standing => (self.process_next_arg(), Ok(Some(w))),

            other_state => (other_state, Ok(None)),
        };
        self.state = new_state;
        result
    }

    pub fn required_parameter(&mut self, free_standing: bool) -> Result<String, ArgError> {
        self.required_parameter_os(free_standing)
            .and_then(|s| s.into_string().map_err(|s| ArgError::InvalidUnicode(s)))
    }

    pub fn required_parameter_os(&mut self, free_standing: bool) -> Result<OsString, ArgError> {
        match self.parameter_os(free_standing) {
            Ok(Some(s)) => Ok(s),
            Ok(None) => Err(ArgError::ParameterMissing(
                self.flag_yielded
                    .as_ref()
                    .cloned()
                    .expect("should only be called after flag"),
            )),
            Err(e) => Err(e),
        }
    }

    pub fn take_flag(&mut self, skipped: &mut Vec<String>) -> Result<Option<&str>, ArgError> {
        loop {
            match self.peek_item()? {
                Item::Flag(_) => break,
                Item::Word(w) => skipped.push(String::from(w)),
                Item::End => return Ok(None),
            }
        }
        match self.take_item_os()? {
            ItemOs::Flag(f) => return Ok(Some(f)),
            _ => unreachable!(),
        }
    }

    pub fn take_flag_os(&mut self, skipped: &mut Vec<OsString>) -> Result<Option<&str>, ArgError> {
        loop {
            match self.peek_item_os()? {
                ItemOs::Flag(_) => break,
                ItemOs::Word(w) => skipped.push(OsString::from(w)),
                ItemOs::End => return Ok(None),
            }
        }
        match self.take_item_os()? {
            ItemOs::Flag(f) => return Ok(Some(f)),
            _ => unreachable!(),
        }
    }
}

fn state_shorts(letters: &str, tail: OsString) -> Option<State> {
    if let Some(first) = letters.chars().next() {
        let mut flag = String::with_capacity(5);
        flag.push('-');
        flag.push(first);
        let letters = letters.to_string();
        Some(State::Shorts {
            flag,
            letters,
            tail,
        })
    } else if tail.is_empty() {
        None
    } else {
        Some(Failed(ArgError::InvalidUnicode(tail)))
    }
}
