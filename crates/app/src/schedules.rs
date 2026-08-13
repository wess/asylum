//! Firing schedules: work that starts with nobody watching.
//!
//! The store decides *when* (`store::schedule::advance`, tested there). This
//! decides *what happens* — creating the task and fanning it out through the
//! same path a person uses, so a scheduled run is an ordinary run in every way
//! that matters afterwards: same worktrees, same board, same review, same
//! notifications.
//!
//! Nothing here is special-cased on the far side. A run you find waiting in the
//! morning should be indistinguishable from one you started yourself, because
//! the whole point is that you review it the same way.

use gpui::{Context, Window};

use crate::state::{now, Root};

/// How often to look for due schedules.
///
/// A minute, not a second. The finest cadence anyone sets is measured in
/// minutes, and a schedule that fires up to a minute late is not a schedule
/// anybody notices being late — whereas a tick that wakes the app every second
/// to run a query is a battery cost paid all day for nothing.
pub const TICK: std::time::Duration = std::time::Duration::from_secs(60);

impl Root {
    /// Fire every schedule that has come due.
    ///
    /// Returns how many fired, which the caller logs. Errors are swallowed per
    /// schedule rather than aborting the sweep: one project with a broken
    /// repository must not stop every other schedule in the app.
    pub fn fire_due_schedules(&mut self, window: &mut Window, cx: &mut Context<Self>) -> usize {
        let at = now();
        let Ok(due) = self.db.due_schedules(at) else {
            return 0;
        };
        let mut fired = 0;
        for schedule in due {
            // Marked *before* dispatching, deliberately. A fan-out takes time
            // and can fail, and a schedule that only advances on success is due
            // again on the next tick — one broken run becoming a new one every
            // minute, forever, while nobody is watching. That is the failure
            // mode unattended work has to be designed against.
            if self.db.mark_schedule_fired(schedule.id, at).is_err() {
                continue;
            }
            if self.run_schedule(&schedule, window, cx) {
                fired += 1;
            }
        }
        fired
    }

    /// Create the task for one schedule and fan it out.
    ///
    /// Runs against the schedule's own project, which is not necessarily the
    /// one currently open — a nightly job must not quietly retarget itself at
    /// whatever repository you happened to leave selected.
    fn run_schedule(
        &mut self,
        schedule: &store::Schedule,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Ok(project) = self.db.project(schedule.project_id) else {
            return false;
        };
        // A repository that is not trusted has not been given permission to run
        // its own commands, and a scheduled run is the *least* supervised
        // moment to make an exception.
        let title = if schedule.title.trim().is_empty() {
            "Scheduled run".to_string()
        } else {
            schedule.title.clone()
        };
        let Ok(task) = self
            .db
            .create_task(project.id, &title, &schedule.prompt, now())
        else {
            return false;
        };

        let agents = schedule.agent_ids();
        let previous_project = self.project_id;
        let previous_fanout = self.fanout.clone();

        self.project_id = Some(project.id);
        self.task_id = Some(task.id);
        if !agents.is_empty() {
            self.fanout = agents;
        }
        self.run_fanout(window, cx);

        // Put the selection back. A schedule firing while somebody is working
        // must not move them to another project mid-sentence.
        self.project_id = previous_project;
        self.fanout = previous_fanout;
        self.push_notification(
            "run_started",
            "Scheduled run started",
            &format!("{title} on {}", project.name),
            None,
        );
        true
    }
}
