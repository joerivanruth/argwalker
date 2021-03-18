use std::{ffi::OsStr, fmt};

use crate::ArgError;

/**
Item returned from [`ArgWalker::take_item`].
*/
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Item<'a> {
    Flag(&'a str),
    Word(&'a str),
}

/**
Item returned from [`ArgWalker::take_item_os`].
*/
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemOs<'a> {
    Flag(&'a str),
    Word(&'a OsStr),
}

impl fmt::Display for Item<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Item::Flag(flag) => flag.fmt(f),
            Item::Word(word) => word.fmt(f),
        }
    }
}

impl fmt::Display for ItemOs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemOs::Flag(flag) => flag.fmt(f),
            ItemOs::Word(word) => word.to_string_lossy().fmt(f),
        }
    }
}

pub fn unicode_item(item: ItemOs<'_>) -> Result<Item<'_>, ArgError> {
    match item {
        ItemOs::Flag(f) => Ok(Item::Flag(f)),
        ItemOs::Word(w) => match w.to_str() {
            Some(s) => Ok(Item::Word(s)),
            None => Err(ArgError::InvalidUnicode(std::ffi::OsString::from(w))),
        },
    }
}

pub fn unicode_item_option(item_opt: Option<ItemOs<'_>>) -> Result<Option<Item<'_>>, ArgError> {
    match item_opt {
        None => Ok(None),
        Some(item) => unicode_item(item).map(Some),
    }
}
