Splitargs - helper library for parsing command line arguments
=============================================================

Helper library for command line argument parsing.

Allows you to conveniently iterate over flags and other
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
use argwalker::{ArgWalker,ArgError,Item};
fn main() -> Result<(), ArgError> {
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
   Ok(())
}
```