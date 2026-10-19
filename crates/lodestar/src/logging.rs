//! Tracing setup: a pretty `info` console subscriber first, swapped for the
//! configured level and format once config has loaded.

use tracing_subscriber::{
    EnvFilter, Layer, Registry,
    layer::{Layered, SubscriberExt},
    reload,
    util::SubscriberInitExt,
};

use crate::config::{Log, LogFormat};

type Formatter = Box<dyn Layer<Filtered> + Send + Sync>;
type Filtered = Layered<reload::Layer<EnvFilter, Registry>, Registry>;

pub struct Logging {
    filter: reload::Handle<EnvFilter, Registry>,
    format: reload::Handle<Formatter, Filtered>,
}

fn formatter(format: LogFormat) -> Formatter {
    match format {
        LogFormat::Pretty => tracing_subscriber::fmt::layer().pretty().boxed(),
        LogFormat::Json => tracing_subscriber::fmt::layer().json().boxed(),
    }
}

/// Installs the bootstrap subscriber.
pub fn init() -> Logging {
    let (filter_layer, filter) = reload::Layer::new(EnvFilter::new("info"));
    let (format_layer, format) = reload::Layer::new(formatter(LogFormat::Pretty));
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(format_layer)
        .init();
    Logging { filter, format }
}

impl Logging {
    /// Switches to the configured level and format. The level was validated
    /// when the config loaded.
    pub fn apply(&self, log: &Log) {
        let filter = EnvFilter::try_new(&log.level).unwrap_or_else(|_| EnvFilter::new("info"));
        let result = self
            .filter
            .reload(filter)
            .and_then(|()| self.format.reload(formatter(log.format)));
        if let Err(error) = result {
            tracing::warn!(%error, "could not apply log configuration");
        }
    }
}
