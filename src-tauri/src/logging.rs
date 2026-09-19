//! Tracing to a rotating file, plus the console during development.
//!
//! The on-disk log is the diagnostic trail for failures the user only notices
//! after the fact ("it stopped working at some point this evening"); the
//! in-app journal is the live view.

use std::path::Path;

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initialise tracing. The returned guard flushes the file on drop and must be
/// kept alive for the lifetime of the process.
pub fn init(directory: &Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_env("ZAAPY_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = std::fs::create_dir_all(directory)
        .ok()
        .map(|()| tracing_appender::rolling::daily(directory, "zaapy.log"))
        .map(tracing_appender::non_blocking);

    match file_layer {
        Some((writer, guard)) => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(writer),
                )
                .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
                .init();
            Some(guard)
        }
        None => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
                .init();
            tracing::warn!(?directory, "no log directory; logging to stderr only");
            None
        }
    }
}
