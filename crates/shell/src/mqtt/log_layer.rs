//! `tracing` layer that publishes log records to MQTT.
//!
//! Every record visits two sinks:
//! 1. the normal fmt-to-stdout layer installed by `main::init_tracing`;
//! 2. this layer, which formats each record as a small JSON blob and
//!    publishes it (non-retained, QoS 0) to
//!    `<topic_root>/log/<LEVEL>/<target>`.
//!
//! The publisher is an [`rumqttc::AsyncClient`] borrowed behind an
//! `Arc<Mutex<>>` via an mpsc forwarding task, so the tracing layer
//! itself is sync (as required by `tracing-subscriber`) while the
//! actual publish happens on the tokio runtime.

use std::fmt::Write as _;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use rumqttc::{AsyncClient, QoS};
use tokio::sync::mpsc;
use tracing::field::{Field, Visit};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

/// Create the mpsc channel used between the tracing layer (producer)
/// and the background publisher task (consumer). Returns both halves
/// so the caller can install the layer before MQTT connects.
#[must_use]
pub fn log_channel() -> (mpsc::Sender<LogRecord>, mpsc::Receiver<LogRecord>) {
    mpsc::channel(256)
}

/// Spawn the background task that drains `rx` and publishes each
/// record to MQTT. Call AFTER the broker connection is established.
pub fn spawn_log_publisher(
    mut rx: mpsc::Receiver<LogRecord>,
    client: AsyncClient,
    topic_root: String,
) {
    tokio::spawn(async move {
        while let Some(record) = rx.recv().await {
            let topic = format!(
                "{topic_root}/log/{level}/{target}",
                level = record.level,
                target = record.target,
            );
            let payload = record.to_json();
            // Don't use tracing! here — it'd loop back into this same task
            // via MqttLogLayer and could wedge under broker stalls.
            match tokio::time::timeout(
                Duration::from_secs(1),
                client.publish(&topic, QoS::AtMostOnce, false, payload.as_bytes()),
            )
            .await
            {
                Ok(Ok(())) => {}
                Ok(Err(e)) => eprintln!("mqtt log publish failed on {topic}: {e}"),
                Err(_) => eprintln!("mqtt log publish stuck >1s on {topic}; dropping log record"),
            }
        }
    });
}

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub at: DateTime<Utc>,
    pub level: &'static str,
    pub target: String,
    pub message: String,
}

impl LogRecord {
    fn to_json(&self) -> String {
        // Hand-written so we don't pull serde_json into the hot path.
        // Escapes just the minimum JSON specials we'd see in a log
        // message.
        let mut s = String::with_capacity(self.message.len() + 64);
        s.push('{');
        let _ = write!(
            &mut s,
            r#""at":"{}","level":"{}","target":"{}","message":"#,
            self.at.to_rfc3339(),
            self.level,
            json_escape(&self.target),
        );
        s.push('"');
        s.push_str(&json_escape(&self.message));
        s.push('"');
        s.push('}');
        s
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '"' => out.push_str(r#"\""#),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(&mut out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

// --- Layer implementation ---------------------------------------------------

#[derive(Debug)]
pub struct MqttLogLayer {
    tx: mpsc::Sender<LogRecord>,
}

impl MqttLogLayer {
    #[must_use]
    pub fn new(tx: mpsc::Sender<LogRecord>) -> Self {
        Self { tx }
    }
}

impl<S: Subscriber> Layer<S> for MqttLogLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let meta = event.metadata();
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let record = LogRecord {
            at: SystemTime::now().into(),
            level: meta.level().as_str(),
            target: meta.target().to_string(),
            message: visitor.message,
        };
        // try_send: don't block tracing on a full queue. tokio's
        // `mpsc::Sender::try_send` drops the NEW record on Full (not
        // the oldest). So a flood loses the peak-incident lines, not
        // the prelude — acceptable trade for non-blocking tracing;
        // the stdout subscriber still captures everything.
        let _ = self.tx.try_send(record);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            let _ = write!(&mut self.message, "{value:?}");
        } else if !self.message.is_empty() {
            let _ = write!(&mut self.message, " {}={value:?}", field.name());
        } else {
            let _ = write!(&mut self.message, "{}={value:?}", field.name());
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message.push_str(value);
        } else if !self.message.is_empty() {
            let _ = write!(&mut self.message, " {}={value}", field.name());
        } else {
            let _ = write!(&mut self.message, "{}={value}", field.name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_escape_handles_specials() {
        assert_eq!(json_escape("hello"), "hello");
        assert_eq!(json_escape(r#"a"b"#), r#"a\"b"#);
        assert_eq!(json_escape("a\nb"), r"a\nb");
        assert_eq!(json_escape(r"a\b"), r"a\\b");
    }

    #[test]
    fn log_record_json_roundtrip_is_well_formed() {
        let record = LogRecord {
            at: DateTime::parse_from_rfc3339("2026-04-22T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: "INFO",
            target: "victron_controller_shell::mqtt".to_string(),
            message: "hello world".to_string(),
        };
        let json = record.to_json();
        // Feed through serde_json to confirm it parses.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["level"], "INFO");
        assert_eq!(v["message"], "hello world");
        assert_eq!(v["target"], "victron_controller_shell::mqtt");
    }

    #[test]
    fn log_record_escapes_quotes_in_message() {
        let record = LogRecord {
            at: DateTime::parse_from_rfc3339("2026-04-22T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            level: "WARN",
            target: "test".to_string(),
            message: "he said \"hi\" and left".to_string(),
        };
        let json = record.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["message"], "he said \"hi\" and left");
    }
}
