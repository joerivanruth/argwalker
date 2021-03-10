#![allow(dead_code)]

use std::ffi::{OsStr, OsString};

#[cfg(windows)]
fn to_wide(s: &OsStr) -> Vec<u16> {
    std::os::unix::ffi::OsStrExt::encode_wide(s).collect()
}

#[cfg(windows)]
fn from_wide(wide: &[u16]) -> OsString {
    std::os::unix::ffi::OsStringExt::from_wide(wide)
}

#[cfg(not(windows))]
fn to_wide(s: &OsStr) -> Vec<u16> {
    let _ = s;
    unimplemented!("to_wide only implemented on Windows")
}

#[cfg(not(windows))]
fn from_wide(wide: &[u16]) -> OsString {
    let _ = wide;
    unimplemented!("from_wide only implemented on Windows")
}

pub fn split_valid(s: &OsStr) -> (String, OsString) {
    let wide = to_wide(s);

    let valid_to = find_first_invalid(&wide).unwrap_or(wide.len());
    let valid_head = String::from_utf16(&wide[..valid_to]).unwrap();
    let invalid_tail = from_wide(&wide[valid_to..]);

    (valid_head, invalid_tail)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Utf16Group {
    LowSurrogate,
    HighSurrogate,
    Rest,
}

impl Utf16Group {
    fn of(unit: u16) -> Self {
        match unit {
            0x0000..=0xD7FF => Utf16Group::Rest,
            0xD800..=0xDBFF => Utf16Group::HighSurrogate,
            0xDC00..=0xDFFF => Utf16Group::LowSurrogate,
            0xE000..=0xFFFF => Utf16Group::Rest,
        }
    }
}

fn find_first_invalid(units: &[u16]) -> Option<usize> {
    use Utf16Group::*;

    if units.is_empty() {
        return None;
    }

    if Utf16Group::of(units[0]) == LowSurrogate {
        return Some(0);
    }

    for (i, pair) in units.windows(2).enumerate() {
        let first = Utf16Group::of(pair[0]);
        let second = Utf16Group::of(pair[1]);
        match (first, second) {
            (HighSurrogate, LowSurrogate) => {}
            (HighSurrogate, _other) => return Some(i),
            (_other, LowSurrogate) => return Some(i + 1),
            (_, _) => {}
        }
    }

    let last_idx = units.len() - 1;
    if Utf16Group::of(units[last_idx]) == HighSurrogate {
        return Some(last_idx);
    }

    None
}

#[test]
fn test_find_first_invalid() {
    let ok = &[0x0000, 0x0040, 0xD7FF, 0xE000, 0xFFFF];
    let hi = &[0xD800, 0xD840, 0xDBFF];
    let lo = &[0xDC00, 0xDC40, 0xDFFF];

    fn generate(pattern: &[&[u16]], buffer: &mut Vec<u16>, output: &mut Vec<Vec<u16>>) {
        if pattern.is_empty() {
            output.push(buffer.clone());
            return;
        }
        let first = pattern[0];
        let rest = &pattern[1..];
        for &u in first {
            buffer.push(u);
            generate(rest, buffer, output);
            buffer.pop().unwrap();
        }
    }

    fn verify(pattern: &[&[u16]], expected: Option<usize>) {
        let mut buffer: Vec<u16> = vec![];
        let mut testcases: Vec<Vec<u16>> = vec![];
        generate(pattern, &mut buffer, &mut testcases);
        for testcase in testcases {
            let mut text = String::new();
            for u in &testcase {
                use std::fmt::Write;
                write!(text, "{:04x} ", u).unwrap();
            }
            let solution = find_first_invalid(&testcase);

            assert_eq!(
                solution, expected,
                "find_first_invalid failed for input {:?}",
                text
            );
        }
    }

    verify(&[], None);

    verify(&[ok], None);
    verify(&[hi], Some(0));
    verify(&[lo], Some(0));

    // all pairs
    verify(&[ok, ok], None);
    verify(&[ok, hi], Some(1));
    verify(&[ok, lo], Some(1));
    verify(&[hi, ok], Some(0));
    verify(&[hi, hi], Some(0));
    verify(&[hi, lo], None);
    verify(&[lo, ok], Some(0));
    verify(&[lo, hi], Some(0));
    verify(&[lo, lo], Some(0));

    // all pairs, with something valid after
    verify(&[ok, ok, ok], None);
    verify(&[ok, hi, ok], Some(1));
    verify(&[ok, lo, ok], Some(1));
    verify(&[hi, ok, ok], Some(0));
    verify(&[hi, hi, ok], Some(0));
    verify(&[hi, lo, ok], None);
    verify(&[lo, ok, ok], Some(0));
    verify(&[lo, hi, ok], Some(0));
    verify(&[lo, lo, ok], Some(0));
    //
    verify(&[ok, ok, hi, lo], None);
    verify(&[ok, hi, hi, lo], Some(1));
    verify(&[ok, lo, hi, lo], Some(1));
    verify(&[hi, ok, hi, lo], Some(0));
    verify(&[hi, hi, hi, lo], Some(0));
    verify(&[hi, lo, hi, lo], None);
    verify(&[lo, ok, hi, lo], Some(0));
    verify(&[lo, hi, hi, lo], Some(0));
    verify(&[lo, lo, hi, lo], Some(0));

    // all pairs, with something valid before
    verify(&[ok, ok, ok], None);
    verify(&[ok, ok, hi], Some(2));
    verify(&[ok, ok, lo], Some(2));
    verify(&[ok, hi, ok], Some(1));
    verify(&[ok, hi, hi], Some(1));
    verify(&[ok, hi, lo], None);
    verify(&[ok, lo, ok], Some(1));
    verify(&[ok, lo, hi], Some(1));
    verify(&[ok, lo, lo], Some(1));
    //
    verify(&[hi, lo, ok, ok], None);
    verify(&[hi, lo, ok, hi], Some(3));
    verify(&[hi, lo, ok, lo], Some(3));
    verify(&[hi, lo, hi, ok], Some(2));
    verify(&[hi, lo, hi, hi], Some(2));
    verify(&[hi, lo, hi, lo], None);
    verify(&[hi, lo, lo, ok], Some(2));
    verify(&[hi, lo, lo, hi], Some(2));
    verify(&[hi, lo, lo, lo], Some(2));
}
