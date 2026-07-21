use std::path::Path;

use super::*;

#[test]
fn joins_logs_onto_the_data_dir() {
    assert_eq!(
        join(Path::new("/x/y/asylum")),
        Path::new("/x/y/asylum/logs")
    );
}

#[test]
fn parses_known_level_names_case_insensitively() {
    assert_eq!(parse_level(Some("debug")), LevelFilter::DEBUG);
    assert_eq!(parse_level(Some("DEBUG")), LevelFilter::DEBUG);
    assert_eq!(parse_level(Some("Warn")), LevelFilter::WARN);
    assert_eq!(parse_level(Some("trace")), LevelFilter::TRACE);
    assert_eq!(parse_level(Some("error")), LevelFilter::ERROR);
    assert_eq!(parse_level(Some("info")), LevelFilter::INFO);
}

#[test]
fn defaults_to_info_when_unset_blank_or_unrecognized() {
    assert_eq!(parse_level(None), LevelFilter::INFO);
    assert_eq!(parse_level(Some("")), LevelFilter::INFO);
    assert_eq!(parse_level(Some("   ")), LevelFilter::INFO);
    assert_eq!(parse_level(Some("not-a-level")), LevelFilter::INFO);
}

#[test]
fn trims_surrounding_whitespace() {
    assert_eq!(parse_level(Some("  debug  ")), LevelFilter::DEBUG);
}
