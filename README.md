Splitargs - helper library for parsing command line arguments
=============================================================

This is not a full featured library.  For example, it does not automatically
generate colorized help messages, etc.  Instead, this library focusses on the
basics. In particular,

- splitting single-dash options into separate flags: `ls -lh` is equivalent to
  '-l', '-h';

- unless they have an argument: `sort -t,` is equivalent to `sort
  --field-separator=,`;

- likewise, with long options, the = is often optional: `--field-separator=,` is
  then equivalent to `--field-separator ,`;

- in Rust, paths should be kept in an `OsString` rather than a `String` because
  the latter is required to decode as valid UTF-8. However, `OsString` is very
  inconvenient to work with so whenever possible it's preferrable to work with
  regular strings.


State machine
-------------

Struct `SplitArgs` keeps track of a list of (partially) unprocessed arguments,
and a State.  The State is one of the following:

- `Finished`. All arguments have been processed.

- `BeforeDouble`.  Next argument starts with two dashes.

- `BeforeSingle`.  Next argument starts with a single dash.

- `BeforeWord`.  Next argument does not start with a dash.

- `Splitting`.  The parser has returned one or more letters from a single-dash
  argument such as '-ltr'. Some letters are remaining and it's still unknown
  whether the remainder is more flags or an argument to the latest returned
  flag.

- `LongArg`.  The parser has return a long (double-dash) flag which had an
  argument that is yet to be returned.  Example: `--field-separator=,`.

Methods:

- `item_os() -> 