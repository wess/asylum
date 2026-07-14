use super::*;

#[test]
fn empty_and_missing_programs_are_not_ready() {
    assert_eq!(find_program(""), None);
    assert_eq!(find_program("asylum-program-that-does-not-exist"), None);
}

#[cfg(unix)]
#[test]
fn absolute_executable_is_ready() {
    assert_eq!(
        find_program("/bin/sh"),
        Some(std::path::PathBuf::from("/bin/sh"))
    );
}
