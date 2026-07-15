//! Run an agent on a pty and supervise its lifecycle.
//!
//! This is the ADE's execution engine: hand it an [`agent::SpawnSpec`] and it
//! launches the agent on a real pseudo-terminal (via the embedded headless
//! terminal core), pumps its output into a `vt` grid you can snapshot, and
//! tracks when it exits. The gpui app renders interactive panes with
//! embedded terminal panes; the `runner` is the headless supervisor used for
//! background runs, the CLI, and tests - anywhere there is no window.
//!
//! ```no_run
//! use runner::Runner;
//! # let spec = agent::SpawnSpec { program: "echo".into(), args: vec!["hi".into()], cwd: ".".into(), stdin: None, env: Vec::new() };
//! let run = Runner::start(&spec).unwrap();
//! run.wait(std::time::Duration::from_secs(5));
//! println!("{}", run.screen_text());
//! ```

use std::io;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub mod scrollback;

use agent::SpawnSpec;
use libsinclair::pty::SpawnOptions;
use libsinclair::terminal::Event;
use libsinclair::{Session, SessionOptions};

/// Where a run is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The agent process is live.
    Running,
    /// The agent exited; carries its unix exit code (`None` if killed by signal
    /// or the code was unavailable).
    Exited(Option<i32>),
}

impl State {
    /// True once the run has reached a terminal state.
    pub fn is_terminal(self) -> bool {
        matches!(self, State::Exited(_))
    }

    /// The exit code, if the run exited with one.
    pub fn exit_code(self) -> Option<i32> {
        match self {
            State::Exited(code) => code,
            State::Running => None,
        }
    }

    /// True when the run exited cleanly (code 0).
    pub fn succeeded(self) -> bool {
        matches!(self, State::Exited(Some(0)))
    }
}

/// A supervised agent run.
pub struct Runner {
    session: Session,
    state: Arc<Mutex<State>>,
    /// The event-pump thread. Joined on shutdown.
    pump: Option<JoinHandle<()>>,
}

impl Runner {
    /// Launch `spec` on a pty and begin supervising it.
    pub fn start(spec: &SpawnSpec) -> io::Result<Runner> {
        Self::start_sized(spec, 120, 40)
    }

    /// Launch with an explicit terminal size.
    pub fn start_sized(spec: &SpawnSpec, cols: usize, rows: usize) -> io::Result<Runner> {
        let mut argv = Vec::with_capacity(spec.args.len() + 1);
        argv.push(spec.program.clone());
        argv.extend(spec.args.iter().cloned());

        let mut options = SessionOptions::command(argv);
        options.cols = cols;
        options.rows = rows;
        options.spawn = SpawnOptions {
            cwd: Some(spec.cwd.clone().into()),
            env: spec.env.clone(),
            ..options.spawn
        };

        let (session, events) = Session::spawn(options)?;

        // Stdin-delivery agents receive the prompt as typed input.
        if let Some(input) = &spec.stdin {
            session.write(input.as_bytes())?;
            session.write(b"\n")?;
        }

        let state = Arc::new(Mutex::new(State::Running));
        let pump = {
            let state = state.clone();
            thread::spawn(move || {
                for event in events {
                    if let Event::Exit(code) = event {
                        *state.lock().unwrap() = State::Exited(code);
                        break;
                    }
                }
                // Channel closed without an explicit Exit → treat as exited.
                let mut guard = state.lock().unwrap();
                if !guard.is_terminal() {
                    *guard = State::Exited(None);
                }
            })
        };

        Ok(Runner {
            session,
            state,
            pump: Some(pump),
        })
    }

    /// The current lifecycle state.
    pub fn state(&self) -> State {
        *self.state.lock().unwrap()
    }

    /// True while the agent is still running.
    pub fn is_running(&self) -> bool {
        !self.state().is_terminal()
    }

    /// Send bytes to the agent's stdin (a follow-up prompt, a keystroke).
    pub fn write(&self, bytes: &[u8]) -> io::Result<()> {
        self.session.write(bytes)
    }

    /// Snapshot the visible terminal grid as text - the agent's current output.
    pub fn screen_text(&self) -> String {
        self.session.with_term(|term| {
            let rows = term.rows();
            (0..rows)
                .map(|r| term.row_text(r))
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_string()
        })
    }

    /// The terminal history as text - every non-empty row of the grid, top to
    /// bottom. Persisted via [`scrollback::save`] so it can be restored on a
    /// later launch.
    pub fn history_text(&self) -> String {
        self.session.with_term(|term| {
            let rows = term.rows();
            (0..rows)
                .map(|r| term.row_text(r))
                .collect::<Vec<_>>()
                .join("\n")
                .trim_end()
                .to_string()
        })
    }

    /// Block until the run reaches a terminal state or `timeout` elapses.
    /// Returns the final [`State`] (still `Running` if it timed out).
    pub fn wait(&self, timeout: Duration) -> State {
        let deadline = Instant::now() + timeout;
        loop {
            let state = self.state();
            if state.is_terminal() || Instant::now() >= deadline {
                return state;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// Terminate the agent and release the pty. Consumes the runner.
    pub fn shutdown(mut self) {
        self.session.shutdown();
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
    }
}

#[cfg(test)]
#[path = "../tests/lib.rs"]
mod tests;
