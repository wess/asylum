//! Detect and run verification checks in a run's worktree.

use std::path::PathBuf;

use gpui::Context;

use super::NoticeTone;
use crate::state::Root;

impl Root {
    pub fn run_check_results(&self, run_id: i64) -> Vec<checks::CheckResult> {
        self.db
            .run_checks(run_id)
            .unwrap_or_default()
            .into_iter()
            .map(|result| checks::CheckResult {
                id: result.id,
                status: match result.status.as_str() {
                    "pass" => checks::Status::Pass,
                    "fail" => checks::Status::Fail,
                    _ => checks::Status::Skipped,
                },
                summary: result.summary,
                duration_ms: result.duration_ms as u128,
            })
            .collect()
    }

    pub fn run_checks(&mut self, cx: &mut Context<Self>) {
        let Some(run_id) = self.current_run_id() else {
            self.push_error("No run selected", "Select a run before starting checks.");
            return;
        };
        self.run_checks_for(run_id, cx);
    }

    pub(super) fn run_checks_for(&mut self, run_id: i64, cx: &mut Context<Self>) {
        if !self.checking_runs.insert(run_id) {
            return;
        }
        let Ok(run) = self.db.run(run_id) else {
            self.checking_runs.remove(&run_id);
            self.push_error("Run unavailable", "The selected run no longer exists.");
            return;
        };
        let worktree = PathBuf::from(run.worktree);
        if !worktree.exists() {
            self.checking_runs.remove(&run_id);
            self.push_error(
                "Worktree missing",
                "Restore or retry the run before starting checks.",
            );
            return;
        }
        let job = cx.background_executor().spawn(async move {
            let detected = checks::detect(&worktree);
            checks::run_all(&worktree, &detected)
        });
        cx.spawn(async move |root, cx| {
            let results = job.await;
            root.update(cx, |root, cx| {
                let stored: Vec<store::RunCheck> = results
                    .iter()
                    .map(|result| store::RunCheck {
                        run_id,
                        id: result.id.clone(),
                        status: result.status.as_str().to_string(),
                        summary: result.summary.clone(),
                        duration_ms: result.duration_ms.min(u64::MAX as u128) as u64,
                    })
                    .collect();
                root.checking_runs.remove(&run_id);
                if let Err(error) = root.db.replace_run_checks(run_id, &stored) {
                    root.push_error("Could not save checks", error.to_string());
                } else {
                    let summary = if results.is_empty() {
                        "No checks detected".to_string()
                    } else {
                        format!(
                            "{} check(s), overall {}",
                            results.len(),
                            checks::overall(&results).as_str()
                        )
                    };
                    root.reference_run_notes(
                        run_id,
                        &notes::Reference::checks(run_id, &summary),
                    );
                }
                if results.is_empty() {
                    root.push_notice(
                        NoticeTone::Warning,
                        "No checks detected",
                        "Add project test, lint, or type-check configuration before merging high-risk changes.",
                    );
                } else if checks::overall(&results) == checks::Status::Fail {
                    root.push_error(
                        "Checks failed",
                        "Open Review for the failed command summary, then send fixes to the selected agent.",
                    );
                } else {
                    root.push_notice(
                        NoticeTone::Success,
                        "Checks finished",
                        "Verification results were saved with this run.",
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
