use super::*;

const SAMPLE: &str = "diff --git a/src/main.rs b/src/main.rs
index 111..222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@ fn main() {
 let x = 1;
-let y = 2;
+let y = 3;
+let z = 4;
 println!(\"{x}\");
diff --git a/new.txt b/new.txt
new file mode 100644
index 000..333
--- /dev/null
+++ b/new.txt
@@ -0,0 +1,1 @@
+hello
";

#[test]
fn parses_two_files() {
    let files = parse(SAMPLE);
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "src/main.rs");
    assert_eq!(files[0].status, FileStatus::Modified);
    assert_eq!(files[1].path, "new.txt");
    assert_eq!(files[1].status, FileStatus::Added);
}

#[test]
fn hunk_header_and_line_numbers() {
    let files = parse(SAMPLE);
    let hunk = &files[0].hunks[0];
    assert_eq!(hunk.old_start, 1);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.header, "fn main() {");

    // First body line is context "let x = 1;" at old 1 / new 1.
    let first = &hunk.lines[0];
    assert_eq!(first.kind, LineKind::Context);
    assert_eq!(first.old_no, Some(1));
    assert_eq!(first.new_no, Some(1));

    // The removed line has only an old number, added lines only new numbers.
    let removed = hunk.lines.iter().find(|l| l.kind == LineKind::Removed).unwrap();
    assert_eq!(removed.content, "let y = 2;");
    assert_eq!(removed.old_no, Some(2));
    assert_eq!(removed.new_no, None);
}

#[test]
fn line_stats_counts() {
    let files = parse(SAMPLE);
    assert_eq!(files[0].line_stats(), (2, 1));
    assert_eq!(files[1].line_stats(), (1, 0));
}

#[test]
fn ignores_no_newline_marker() {
    let d = "diff --git a/a b/a
--- a/a
+++ b/a
@@ -1 +1 @@
-old
\\ No newline at end of file
+new
\\ No newline at end of file
";
    let files = parse(d);
    let lines = &files[0].hunks[0].lines;
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].kind, LineKind::Removed);
    assert_eq!(lines[1].kind, LineKind::Added);
}
