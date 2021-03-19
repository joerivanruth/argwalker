use std::{error, ffi::OsString};
use std::fmt;


/**
Error type for `ArgWalker`.
*/
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgError {
    /// Argument could not be decoded as valid Unicode.
    InvalidUnicode(OsString),
    /// Returned by [`ArgWalker::take_item`] and [`ArgWalker::take_item_os`]
    /// if the previous long option has a parameter which has not been
    /// retrieved with [`ArgWalker::parameter`], for example `--fruit=banana`.
    UnexpectedParameter(String),
    /// Returned by [`ArgWalker::parameter`] and [`ArgWalker::parameter_os`]
    /// if no parameter is available, for example on `-f` in  `-f -v`.
    ParameterMissing(String),
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgError::InvalidUnicode(a) => write!(f, "invalid unicode in argument {:?}", a),
            ArgError::UnexpectedParameter(flag) => write!(f, "unexpected parameter for flag {}", flag),
            ArgError::ParameterMissing(flag) => write!(f, "parameter missing for flag {}", flag),
        }
    }
}

impl error::Error for ArgError {}