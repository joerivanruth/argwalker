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

```rust
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

use std::ffi::{OsStr, OsString};

use corewalker::CoreWalker;
use thiserror::Error;

mod item;
use item::unicode_item_option;
pub use item::{Item, ItemOs};

mod corewalker;
mod oschars;

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
    core: CoreWalker,
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
        S: AsRef<OsStr>,
    {
        ArgWalker {
            core: CoreWalker::new(args),
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
        self.peek_item_os().and_then(unicode_item_option)
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
        self.core.upcoming()
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
        self.take_item_os().and_then(unicode_item_option)
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
        self.core.advance()
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
        if self.core.can_parameter() {
            return true;
        }

        if free_standing {
            if let Ok(Some(ItemOs::Word(_))) = self.core.upcoming() {
                return true;
            }
        }

        false
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
        if let Some(p) = self.core.parameter() {
            return Ok(Some(p.to_os_string()));
        }

        if !free_standing {
            return Ok(None);
        }

        let item = match self.core.upcoming()? {
            Some(ItemOs::Word(_)) => self.core.advance(),
            _ => return Ok(None),
        };

        // we know it's a Word, just have to convince the type checker
        if let Ok(Some(ItemOs::Word(w))) = item {
            Ok(Some(w.to_os_string()))
        } else {
            panic!("upcoming said Ok(Some(Word)) but I got {:?}", item)
        }
    }

    pub fn required_parameter(&mut self, free_standing: bool) -> Result<String, ArgError> {
        self.required_parameter_os(free_standing)
            .and_then(|s| s.into_string().map_err(ArgError::InvalidUnicode))
    }

    pub fn required_parameter_os(&mut self, free_standing: bool) -> Result<OsString, ArgError> {
        if let Some(p) = self.parameter_os(free_standing)? {
            return Ok(p);
        }

        if let Some(flag) = self.core.current_flag() {
            Err(ArgError::ParameterMissing(flag.to_string()))
        } else {
            panic!(".required_parameter can only be called right after a flag")
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
            Some(ItemOs::Flag(f)) => Ok(Some(f)),
            _ => unreachable!(),
        }
    }
}
