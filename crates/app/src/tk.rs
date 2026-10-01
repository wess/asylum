//! The tokio runtime the Bot engine runs on, and the bridge into gpui:
//! `spawn` runs a future on tokio and hands back a handle any executor
//! (including gpui's foreground) can await.

use std::future::Future;
use std::sync::OnceLock;

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn runtime() -> &'static tokio::runtime::Runtime {
  RT.get_or_init(|| {
    tokio::runtime::Builder::new_multi_thread()
      .enable_all()
      .thread_name("engine")
      .build()
      .expect("tokio runtime")
  })
}

pub fn spawn<F>(f: F) -> tokio::task::JoinHandle<F::Output>
where
  F: Future + Send + 'static,
  F::Output: Send + 'static,
{
  runtime().spawn(f)
}

/// Run a future on tokio and wait for it from any async context, folding a
/// panicked task into an error.
pub async fn run<F, T>(f: F) -> anyhow::Result<T>
where
  F: Future<Output = anyhow::Result<T>> + Send + 'static,
  T: Send + 'static,
{
  spawn(f).await.map_err(|e| anyhow::anyhow!("task failed: {e}"))?
}
