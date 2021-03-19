use std::{
    ffi::{OsStr, OsString},
    mem,
    ops::Not,
};

use crate::{item::ItemOs, ArgError};

type ArgResult<T> = Result<T, ArgError>;

/// Intermediate representation of a command line argument.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Parsed {
    /// Flags must be valid unicode. Undecodable units are only
    /// allowed after the = of a long parameter, and in non-flags.
    Invalid(OsString),

    /// Fully decodable argument starting with a dash.
    Short { flags: String },

    /// Partially decodable argument starting with a dash. The tail contains
    /// anything from the first undecodable code unit on.
    ShortTail { flags: String, tail: OsString },

    /// Argument starting with a double dash, possibly with a
    /// a parameter delimited with an equals sign.
    Long {
        flag: String,
        parameter: Option<OsString>,
    },

    /// Anything that does not start with a dash, or the special cases
    /// `-` and `--`.
    Arg(OsString),
}

impl Parsed {
    fn new(s: impl AsRef<OsStr>) -> Self {
        let (head, tail) = crate::oschars::split_valid(s.as_ref());
        if (head == "--" || head == "-") && tail.is_empty() {
            Parsed::Arg(OsString::from(head))
        } else if head.starts_with("--") {
            Parsed::parse_long(head, tail)
        } else if head.starts_with('-') {
            if tail.is_empty() {
                Parsed::new_short(head)
            } else {
                Parsed::new_short_tail(head, tail)
            }
        } else {
            Parsed::Arg(OsString::from(s.as_ref()))
        }
    }

    fn new_short(flags: String) -> Self {
        assert!(flags.len() > 1);
        Parsed::Short { flags }
    }

    fn new_short_tail(flags: String, tail: OsString) -> Self {
        assert!(!tail.is_empty());
        Parsed::ShortTail { flags, tail }
    }

    fn parse_long(head: String, tail: OsString) -> Self {
        assert!(head.starts_with("--"));
        let flag;
        let parameter;
        if let Some(idx) = head.find('=') {
            flag = head[..idx].to_string();
            let mut param = OsString::from(&head[idx + 1..]);
            param.push(tail);
            parameter = Some(param);
        } else if head != "--" && tail.is_empty() {
            flag = head[..].to_string();
            parameter = None;
        } else {
            // flag must be all-valid unicode
            let mut s = OsString::from(head);
            s.push(tail);
            return Parsed::Invalid(s);
        }
        Parsed::Long { flag, parameter }
    }
}

#[test]
fn test_parsed() {
    use crate::oschars::bad_text as bad;
    let oss = |s: &str| OsString::from(s);

    assert_eq!(Parsed::new(oss("banana")), Parsed::Arg(oss("banana")));
    assert_eq!(
        Parsed::new(oss("--follow")),
        Parsed::Long {
            flag: "--follow".to_string(),
            parameter: None
        }
    );
    assert_eq!(
        Parsed::new(oss("--fruit=banana")),
        Parsed::Long {
            flag: "--fruit".to_string(),
            parameter: Some(oss("banana"))
        }
    );
    assert_eq!(
        Parsed::new(oss("-fv")),
        Parsed::Short {
            flags: "-fv".to_string(),
        }
    );

    assert_eq!(Parsed::new(oss("")), Parsed::Arg(oss("")));
    assert_eq!(Parsed::new(oss("-")), Parsed::Arg(oss("-")));
    assert_eq!(Parsed::new(oss("--")), Parsed::Arg(oss("--")));
    assert_eq!(
        Parsed::new(oss("---")),
        Parsed::Long {
            flag: "---".to_string(),
            parameter: None
        }
    );

    assert_eq!(Parsed::new(bad("banana")), Parsed::Arg(bad("banana")));
    assert_eq!(Parsed::new(bad("")), Parsed::Arg(bad("")));
    assert_eq!(
        Parsed::new(bad("-f")),
        Parsed::ShortTail {
            flags: "-f".to_string(),
            tail: bad("")
        }
    );
    assert_eq!(
        Parsed::new(bad("--fruit=bana")),
        Parsed::Long {
            flag: "--fruit".to_string(),
            parameter: Some(bad("bana"))
        }
    );

    assert_eq!(
        Parsed::new(bad("-")),
        Parsed::ShortTail {
            flags: "-".to_string(),
            tail: bad("")
        }
    );
    assert_eq!(Parsed::new(bad("--")), Parsed::Invalid(bad("--")));
    assert_eq!(Parsed::new(bad("--flag")), Parsed::Invalid(bad("--flag")));
}

#[derive(Debug, Clone)]
enum State {
    /// The previously returned item, if any, was not a flag. Maybe we are at
    /// the start, or we have just returned a word.
    NoFlag {
        word: OsString,
    },

    /// The previously returned item was a flag, either something like
    /// `--verbose` or the last letter of a short combi such as `-x` out of
    /// `-vx`. It has already been removed from our Vec<Parsed> but we need to
    /// hold on to the text because we returned a reference to it. There was
    /// nothing that could possibly be regarded as a parameter for this flag.
    Flag {
        flag: String,
    },

    /// The previously returned item was a long flag with a parameter, something
    /// like `--fruit=banana`. The item has been removed from our Vec<Parsed>
    /// but we hold on to the text because we returned a reference to it, and
    /// for error messages. We also hold on to the parameter because caller should
    /// ask for it soon. Boolean `taken` is used to keep track of whether this has
    /// happened yet.
    ParmFlag {
        flag: String,
        parameter: OsString,
        taken: bool,
    },

    /// The previously returned item was a short flag that came out of a
    /// short combi. For example, the `-v` out of `-vx`. The remainder of the combi
    /// is still in our Vec<Parsed>, including the leading dash. In the example
    /// above this means that the Vec<Parsed> now starts with `-v`. If the caller
    /// asks for a parameter, we will return the `v`.
    SplitFlag {
        flag: String,
        taken: bool,
    },

    /// The previously returned item was an error.
    ErrorSeen(ArgError),

    EndSeen,

    Initial,
}

impl State {
    fn as_item(&self) -> ArgResult<Option<ItemOs>> {
        use ItemOs::*;
        let flag = match self {
            State::NoFlag { word } => return Ok(Some(Word(word))),
            State::Flag { flag } => flag,
            State::ParmFlag { flag, .. } => flag,
            State::SplitFlag { flag, .. } => flag,
            State::ErrorSeen(err) => return Err(err.clone()),
            State::EndSeen => return Ok(None),
            State::Initial => panic!("as_item should never get invoked while in state Initial"),
        };
        Ok(Some(ItemOs::Flag(flag)))
    }
}

#[derive(Debug, Clone)]
pub struct CoreWalker {
    state: State,
    args: Vec<Parsed>,
    preview_state: State,
}

impl CoreWalker {
    pub fn new<S, T>(args: T) -> Self
    where
        T: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let args: Vec<Parsed> = args.into_iter().map(Parsed::new).collect();
        let state = State::Initial;
        let preview = Self::compute_preview(&State::Initial, args.first());
        CoreWalker {
            args,
            state,
            preview_state: preview,
        }
    }

    pub fn advance(&mut self) -> ArgResult<Option<ItemOs>> {
        let mut st = State::Initial;
        mem::swap(&mut st, &mut self.state);
        self.state = match st {
            State::SplitFlag { flag, taken: true } => {
                assert!(self.args.is_empty().not());
                self.args.remove(0);
                State::Flag { flag }
            }
            s => s,
        };

        let arg = if self.args.is_empty() {
            None
        } else {
            Some(self.args.remove(0))
        };

        let Decision {
            new_state,
            push_back,
        } = decide(&self.state, arg);
        self.state = new_state;
        if let Some(a) = push_back {
            self.args.insert(0, a);
        }

        self.preview_state = Self::compute_preview(&self.state, self.args.first());

        self.state.as_item()
    }

    pub fn upcoming(&self) -> ArgResult<Option<ItemOs>> {
        self.preview_state.as_item()
    }

    fn compute_preview(state: &State, first: Option<&Parsed>) -> State {
        let Decision { new_state, .. } = decide(&state, first.cloned());
        new_state
    }

    pub fn current_flag(&self) -> Option<&str> {
        match &self.state {
            State::NoFlag { .. } => None,
            State::Flag { flag } => Some(&flag),
            State::ParmFlag { flag, .. } => Some(&flag),
            State::SplitFlag { flag, .. } => Some(&flag),
            State::ErrorSeen(_) => None,
            State::EndSeen => None,
            State::Initial => None,
        }
    }

    pub fn can_parameter(&self) -> bool {
        matches!(
            &self.state,
            State::ParmFlag { .. } | State::SplitFlag { .. }
        )
    }

    pub fn parameter(&mut self) -> Option<&OsStr> {
        let mut shift_preview = false;
        let parm = match &mut self.state {
            State::ParmFlag {
                parameter, taken, ..
            } => {
                *taken = true;
                Some(parameter.as_os_str())
            }
            State::SplitFlag { ref mut taken, .. } => {
                assert!(self.args.is_empty().not());
                let parm = match &self.args[0] {
                    Parsed::Short { flags } | Parsed::ShortTail { flags, .. } => {
                        *taken = true;
                        shift_preview = true;
                        &flags[1..]
                    }
                    _ => panic!("am in state SplitFlag without a Short item as args[0]"),
                };
                Some(OsStr::new(parm))
            }
            _ => None,
        };

        if shift_preview {
            self.preview_state = Self::compute_preview(&State::Initial, self.args.get(1));
        }

        parm
    }
}

struct Decision {
    new_state: State,
    push_back: Option<Parsed>,
}

fn decide(state: &State, arg: Option<Parsed>) -> Decision {
    use Parsed::*;
    use State::*;

    match state {
        // Any pending arguments from --flag must be consumed before moving to
        // the next argument
        ParmFlag {
            flag, taken: false, ..
        } => {
            return Decision {
                new_state: ErrorSeen(ArgError::UnexpectedParameter(flag.clone())),
                push_back: arg,
            }
        }

        // sanity check
        SplitFlag { taken: true, .. } => {
            panic!("splitflag taken=true should have been handled before")
        }

        _ => {}
    }

    let arg = match arg {
        Some(a) => a,
        None => {
            return Decision {
                new_state: EndSeen,
                push_back: None,
            }
        }
    };

    match arg {
        Invalid(s) => Decision {
            new_state: ErrorSeen(ArgError::InvalidUnicode(s)),
            push_back: None,
        },

        Long {
            flag,
            parameter: None,
        } => Decision {
            new_state: Flag { flag },
            push_back: None,
        },

        Long {
            flag,
            parameter: Some(parameter),
        } => Decision {
            new_state: ParmFlag {
                flag,
                parameter,
                taken: false,
            },
            push_back: None,
        },

        Arg(word) => Decision {
            new_state: NoFlag { word },
            push_back: None,
        },

        Short { mut flags } => {
            let flag = chop_off(&mut flags);
            if flags == "-" {
                Decision {
                    new_state: Flag { flag },
                    push_back: None,
                }
            } else {
                Decision {
                    new_state: SplitFlag { flag, taken: false },
                    push_back: Some(Parsed::new_short(flags)),
                }
            }
        }

        ShortTail { mut flags, tail } => {
            if flags == "-" {
                let mut flag = OsString::from("-");
                flag.push(tail);
                Decision {
                    new_state: ErrorSeen(ArgError::InvalidUnicode(flag)),
                    push_back: None,
                }
            } else {
                let flag = chop_off(&mut flags);
                Decision {
                    new_state: SplitFlag { flag, taken: false },
                    push_back: Some(Parsed::new_short_tail(flags, tail)),
                }
            }
        }
    }
}

fn chop_off(flags: &mut String) -> String {
    assert!(flags.starts_with('-'));
    let ch = flags.remove(1);
    format!("-{}", ch)
}

#[cfg(test)]
mod tests {
    use super::ItemOs::*;
    use super::*;

    #[test]
    fn test_items() {
        let mut walker = CoreWalker::new(&["-vx", "-f", "foo"]);

        assert_eq!(walker.upcoming(), Ok(Some(Flag("-v"))));
        assert_eq!(walker.advance(), Ok(Some(Flag("-v"))));

        // consume the x as a parameter
        assert_eq!(walker.can_parameter(), true);
        let mut walker2 = walker.clone();
        assert_eq!(walker2.parameter(), Some(OsString::from("x").as_os_str()));
        assert_eq!(walker2.upcoming(), Ok(Some(Flag("-f"))));
        assert_eq!(walker2.advance(), Ok(Some(Flag("-f"))));

        // consume the x as flag -x
        assert_eq!(walker.upcoming(), Ok(Some(Flag("-x"))));
        assert_eq!(walker.advance(), Ok(Some(Flag("-x"))));

        // nothing behind the x
        assert_eq!(walker.can_parameter(), false);
        let mut walker2 = walker.clone();
        assert_eq!(walker2.parameter(), None);
        assert_eq!(walker2.upcoming(), Ok(Some(Flag("-f"))));
        assert_eq!(walker2.advance(), Ok(Some(Flag("-f"))));

        assert_eq!(walker.upcoming(), Ok(Some(Flag("-f"))));
        assert_eq!(walker.advance(), Ok(Some(Flag("-f"))));

        // before attempting to take the (nonexistent) parameter, foo is upcoming
        assert_eq!(
            walker.upcoming(),
            Ok(Some(Word(OsString::from("foo").as_os_str())))
        );
        assert_eq!(walker.can_parameter(), false);
        assert_eq!(walker.parameter(), None);

        // after the attempt, foo is still upcoming
        assert_eq!(
            walker.upcoming(),
            Ok(Some(Word(OsString::from("foo").as_os_str())))
        );

        // after foo we find eof
        assert_eq!(
            walker.advance(),
            Ok(Some(Word(OsString::from("foo").as_os_str())))
        );
        assert_eq!(walker.upcoming(), Ok(None));
        assert_eq!(walker.advance(), Ok(None));

        // and it remains eof
        assert_eq!(walker.upcoming(), Ok(None));
        assert_eq!(walker.advance(), Ok(None));
        assert_eq!(walker.upcoming(), Ok(None));
        assert_eq!(walker.advance(), Ok(None));
        assert_eq!(walker.upcoming(), Ok(None));
        assert_eq!(walker.advance(), Ok(None));
        assert_eq!(walker.upcoming(), Ok(None));
        assert_eq!(walker.advance(), Ok(None));
    }
}
