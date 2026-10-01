//! Record the build date (`ASYLUM_BUILD_DATE`, `YYYY-MM-DD`) for the
//! Update required check: a build more than 14 days older than the latest
//! release must update.

fn main() {
  println!("cargo:rerun-if-changed=../../.git/HEAD");
  let date = std::process::Command::new("date")
    .arg("+%Y-%m-%d")
    .output()
    .ok()
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim().to_string())
    .unwrap_or_else(|| "2026-01-01".into());
  println!("cargo:rustc-env=ASYLUM_BUILD_DATE={date}");
}
