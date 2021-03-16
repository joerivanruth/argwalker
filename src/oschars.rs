// Always include both oschars_unix and oschars_windows so they both get
// type checked as much as possible.
// Then pick the appropriate implementation.
mod oschars_unix;
mod oschars_windows;

#[cfg(unix)]
pub use oschars_unix::split_valid;

#[cfg(windows)]
pub use oschars_windows::split_valid;
