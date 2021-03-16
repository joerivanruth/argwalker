/*!
Helper library for command line argument parsing.

Struct [`ArgWalker`] allows you to conveniently iterate over flags and other
options. It does not provide higher level features such as parsing into structs
or automatically generating help text. Instead, it only provides the following
services:

1) Splitting combined single-dash flags such as `-xvf` into separate flags `-x`,
   `-v` and `-f`.

2) Dealing with flags with arguments such as `-fbanana` or `--fruit=banana`.
   The latter may or may not be equivalent with `--fruit banana`.

3) Correctly dealing with non-unicode arguments such as filenames, while
   still working with regular strings wherever possible.

The latter is necessary because Rust strings must be valid UTF-8 but on Unix,
filenames can contain arbitrary byte sequences which are not necessarily
UTF-8, while on Windows, filenames are composed of 16 bit sequences that
usually but not necessarily can be decoded as UTF-16.

# Example

```
# use argwalker::{ArgWalker,ArgError,Item};
# fn main() -> Result<(), ArgError> {
    let mut w = ArgWalker::new(&["eat", "file1", "-vfbanana", "file2", "file3"]);

    assert_eq!(w.take_item(), Ok(Some(Item::Word("eat"))));

    let mut verbose = false;
    let mut fruit = None;
    let mut argcount = 0;
    while let Some(item) = w.take_item()? {
        match item {
            Item::Flag("-v") => verbose = true,
            Item::Flag("-f") => fruit = Some(w.required_parameter(true)?),
            Item::Word(w) => argcount += 1,
            x => panic!("unexpected argument {}. Usage: bla bla bla", x)
        }
    }
    assert_eq!(verbose, true);
    assert_eq!(fruit, Some("banana".to_string()));
    assert_eq!(argcount, 3);
#    Ok(())
# }
```

*/

use std::default::Default;
use std::ffi::{OsStr, OsString};
use std::mem;
use std::str;
use std::vec;

use thiserror::Error;

mod item;

use item::{unicode_item_option_result, Item, ItemOs};

// Always include both oschars_unix and oschars_windows so they both get
// type checked as much as possible.
// Then pick the appropriate implementation.
mod oschars_unix;
mod oschars_windows;
#[cfg(unix)]
use oschars_unix::split_valid;
#[cfg(windows)]
use oschars_windows::split_valid;

/**
Command line argument helper.

Created from a sequence of command line arguments.
Every call to [`.take_item()`][ArgWalker::take_item] doles out another flag or argument.
Multi-letter arguments that start with a single dash are split into separate
single letter flags, for example `-vf` becomes `-v` `-f`.
Call [`.parameter()`][ArgWalker::parameter] to obtain the remaining letters as
a string instead of splitting them into options.  For example, with `-xfbanana`,
calling this method after receiving the `-f` will return `banana`.

With double-dash flags such as `--fruit=banana`, [`.take_item()`][ArgWalker::take_item] returns `--fruit`.
This must be followed by a call to [`.parameter()`][ArgWalker::parameter].
If [`.parameter()`][ArgWalker::parameter] is not called, the next
call to [`.take_item()`][ArgWalker::take_item] will yield [`ArgError::UnexpectedParameter`].

All [`String`] returning methods have a `_os` variant which returns an [`OsString`] instead.
*/
pub struct ArgWalker {
    next_args: vec::IntoIter<OsString>,
    state: State,
    hold_os: OsString,
    hold: String,
    flag_yielded: Option<String>,
}

/**
Error type for `ArgWalker`.
*/
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ArgError {
    /// Argument could not be decoded as valid Unicode.
    #[error("invalid unicode in argument {0:?}")]
    InvalidUnicode(OsString),
    /// Returned by [`ArgWalker::take_item`] and [`ArgWalker::take_item_os`]
    /// if the previous long option has a parameter which has not been
    /// retrieved with [`ArgWalker::parameter`], for example `--fruit=banana`.
    #[error("unexpected parameter for flag {0}")]
    UnexpectedParameter(String),
    /// Returned by [`ArgWalker::parameter`] and [`ArgWalker::parameter_os`]
    /// if no parameter is available, for example on `-f` in  `-f -v`.
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
    /// Construct a new [`ArgWalker`].
    ///
    /// # Examples
    ///
    /// When testing
    /// ```
    /// # use argwalker::ArgWalker;
    /// let args = ArgWalker::new(&["foo", "bar", "baz"]);
    /// ```
    ///
    /// In production
    /// ```
    /// # use argwalker::ArgWalker;
    /// use std::env;
    /// let args = ArgWalker::new(env::args_os());
    /// ```
    pub fn new<S, T>(args: T) -> Self
    where
        T: IntoIterator<Item = S>,
        OsString: From<S>,
    {
        let arg_vec: Vec<OsString> = args.into_iter().map(OsString::from).collect();
        let mut next_args = arg_vec.into_iter();
        let state = Self::state_from_arg(next_args.next());
        ArgWalker {
            next_args,
            state,
            hold_os: Default::default(),
            hold: Default::default(),
            flag_yielded: None,
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

    /// Look at the upcoming item in [`String`] form without moving on to the next
    ///
    /// # Example
    /// ```
    /// # use argwalker::{ArgWalker,Item};
    /// let mut args = ArgWalker::new(&["--foo", "--bar"]);
    /// assert_eq!(args.peek_item(), Ok(Some(Item::Flag("--foo"))));
    /// assert_eq!(args.peek_item(), Ok(Some(Item::Flag("--foo")))); // didn't change
    /// ```
    pub fn peek_item(&self) -> Result<Option<Item<'_>>, ArgError> {
        unicode_item_option_result(self.peek_item_os())
    }

    /// Look at the upcoming item in [`OsString`] form without moving on to the next
    ///
    /// # Example
    /// ```
    /// # use argwalker::{ArgWalker,ItemOs};
    /// # use std::ffi::OsString;
    /// let mut args = ArgWalker::new(&["foo", "--bar"]);
    /// let foo = OsString::from("foo");
    /// assert_eq!(args.peek_item_os(), Ok(Some(ItemOs::Word(&foo))));
    /// assert_eq!(args.peek_item_os(), Ok(Some(ItemOs::Word(&foo)))); // didn't change
    /// ```
    pub fn peek_item_os(&self) -> Result<Option<ItemOs<'_>>, ArgError> {
        use ItemOs::*;

        match &self.state {
            Long { flag, .. } => Ok(Some(Flag(&flag))),
            LongParm { flag, .. } => Err(ArgError::UnexpectedParameter(flag.to_string())),
            Shorts { flag, .. } => Ok(Some(Flag(&flag))),
            NotOption(w) => Ok(Some(Word(&w))),
            Finished => Ok(None),
            Failed(e) => Err(e.clone()),
        }
    }

    /// Retrieve the upcoming item in [`String`] form and move on to the next
    ///
    /// # Example
    /// ```
    /// # use argwalker::{ArgWalker,Item};
    /// # use std::ffi::OsString;
    /// let mut args = ArgWalker::new(&["foo", "--bar"]);
    /// assert_eq!(args.take_item(), Ok(Some(Item::Word("foo"))));
    /// assert_eq!(args.take_item(), Ok(Some(Item::Flag("--bar"))));
    /// ```
    pub fn take_item(&mut self) -> Result<Option<Item<'_>>, ArgError> {
        unicode_item_option_result(self.take_item_os())
    }

    /// Retrieve the upcoming item in [`OsString`] form and move on to the next
    ///
    /// # Example
    /// ```
    /// # use argwalker::{ArgWalker,ItemOs};
    /// # use std::ffi::OsString;
    /// let mut args = ArgWalker::new(&["foo", "--bar"]);
    /// let foo = OsString::from("foo");
    /// assert_eq!(args.take_item_os(), Ok(Some(ItemOs::Word(&foo))));
    /// assert_eq!(args.take_item_os(), Ok(Some(ItemOs::Flag("--bar"))));
    /// ```
    pub fn take_item_os(&mut self) -> Result<Option<ItemOs<'_>>, ArgError> {
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
                (LongParm { flag, parm: p }, Ok(Some(Flag(&self.hold))))
            }

            Long { flag, parm: None } => {
                self.hold = flag.clone();
                (self.process_next_arg(), Ok(Some(Flag(&self.hold))))
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
                // might be unicode so .len() is not necessarily 2.
                let other_letters = &letters[flag.len() - 1..];
                let st =
                    state_shorts(other_letters, tail).unwrap_or_else(|| self.process_next_arg());
                self.hold = flag;
                (st, Ok(Some(Flag(&self.hold))))
            }

            NotOption(w) => {
                self.hold_os = w;
                (
                    self.process_next_arg(),
                    Ok(Some(Word(&self.hold_os as &OsStr))),
                )
            }

            Finished => (Finished, Ok(None)),

            Failed(e) => (Failed(e.clone()), Err(e)),
        };
        self.state = new_state;

        match result {
            Ok(Some(Flag(f))) => self.flag_yielded = Some(f.to_string()),
            Ok(_) => self.flag_yielded = None,
            _ => {}
        }

        result
    }

    /// Returns `true` if a parameter is available.
    ///
    /// Parameter `free_standing` controls whether a subsequent word will also
    /// be considered a parameter.
    ///
    /// # Examples
    ///
    /// With `free_standing == true`:
    /// ```
    /// # use argwalker::{ArgWalker,Item};
    /// # use Item::*;
    /// let mut args = ArgWalker::new(&["-fbanana"]);
    /// assert_eq!(args.take_item(), Ok(Some(Flag("-f"))));
    /// assert_eq!(args.has_parameter(true), true);
    /// assert_eq!(args.parameter(true), Ok(Some("banana".to_string())));
    ///
    /// let mut args = ArgWalker::new(&["-f", "banana"]);
    /// assert_eq!(args.take_item(), Ok(Some(Flag("-f"))));
    /// assert_eq!(args.has_parameter(true), true);
    /// assert_eq!(args.parameter(true), Ok(Some("banana".to_string())));
    ///
    /// let mut args = ArgWalker::new(&["-f", "-v"]);
    /// assert_eq!(args.take_item(), Ok(Some(Flag("-f"))));
    /// assert_eq!(args.has_parameter(true), false);
    /// assert_eq!(args.take_item(), Ok(Some(Flag("-v"))));
    /// ```
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
                Some(Item::Flag(_)) => break,
                Some(Item::Word(w)) => skipped.push(String::from(w)),
                None => return Ok(None),
            }
        }
        match self.take_item_os()? {
            Some(ItemOs::Flag(f)) => return Ok(Some(f)),
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
