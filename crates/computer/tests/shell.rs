use super::*;

fn spec<'a>(command: &'a str, cwd: &'a Path, place: Place, env: &'a [(String, String)], ms: u64) -> Spec<'a> {
  Spec { command, cwd, place, env, timeout: Duration::from_millis(ms), net: crate::sandbox::Net::Open }
}

#[tokio::test]
async fn runs_in_cwd_and_reports_exit() {
  let dir = tempfile::tempdir().unwrap();
  let out = run(spec("pwd; exit 3", dir.path(), Place::Local, &[], 10_000)).await.unwrap();
  assert_eq!(out.code, Some(3));
  let real = std::fs::canonicalize(dir.path()).unwrap();
  assert!(out.stdout.trim().ends_with(real.file_name().unwrap().to_str().unwrap()));
  assert!(out.render().contains("[exit 3]"));
}

#[tokio::test]
async fn times_out() {
  let dir = tempfile::tempdir().unwrap();
  let out = run(spec("sleep 5", dir.path(), Place::Local, &[], 200)).await.unwrap();
  assert!(out.timed_out);
}

#[tokio::test]
async fn secrets_injected_and_redacted() {
  let dir = tempfile::tempdir().unwrap();
  let env = vec![("API_TOKEN".to_string(), "tok-supersecret".to_string())];
  let out = run(spec("echo $API_TOKEN", dir.path(), Place::Local, &env, 10_000)).await.unwrap();
  assert_eq!(out.stdout.trim(), "[REDACTED]");
}

#[tokio::test]
async fn computer_sandbox_blocks_writes_outside_workspace() {
  if !sandbox::available() {
    return;
  }
  let dir = tempfile::tempdir().unwrap();
  let ws = std::fs::canonicalize(dir.path()).unwrap().join("ws");
  let outside = std::env::var("HOME").unwrap() + "/.asylum-sandbox-probe";
  let cmd = format!("touch inside && touch {outside}; echo done");
  let out = run(spec(&cmd, &ws, Place::Computer, &[], 10_000)).await.unwrap();
  assert!(ws.join("inside").exists());
  assert!(!std::path::Path::new(&outside).exists());
  assert!(out.stderr.contains("Operation not permitted") || out.stdout.contains("done"));
}
