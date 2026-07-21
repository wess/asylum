use super::*;

#[test]
fn formats_a_readable_report() {
    let panic = Panic {
        message: "index out of bounds: the len is 0 but the index is 3",
        location: "src/run.rs:42:9",
        backtrace: "0: run::launch\n1: main",
        version: "0.1.3",
        os: "macos",
    };
    let report = format_report(&panic);
    assert!(report.contains("0.1.3"));
    assert!(report.contains("macos"));
    assert!(report.contains("src/run.rs:42:9"));
    assert!(report.contains("index out of bounds: the len is 0 but the index is 3"));
    assert!(report.contains("run::launch"));
}

#[test]
fn extracts_a_str_payload() {
    let payload: Box<dyn std::any::Any + Send> = Box::new("boom");
    assert_eq!(payload_message(&*payload), "boom");
}

#[test]
fn extracts_a_string_payload() {
    let payload: Box<dyn std::any::Any + Send> = Box::new(String::from("kaboom"));
    assert_eq!(payload_message(&*payload), "kaboom");
}

#[test]
fn falls_back_for_unknown_payload_types() {
    let payload: Box<dyn std::any::Any + Send> = Box::new(42_i32);
    assert_eq!(payload_message(&*payload), "non-string panic payload");
}
