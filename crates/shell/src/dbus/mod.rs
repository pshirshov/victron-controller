//! D-Bus side of the shell.
//!
//! Two directions:
//!
//! - `subscriber` — connects to the Venus system bus, watches the full
//!   set of `(service, path)` pairs the controllers need, and converts
//!   each property change into a core `Event` that is sent down an mpsc
//!   channel.
//!
//! - `writer` — accepts `Effect::WriteDbus` effects from the event loop
//!   and pushes them back to the system bus as `com.victronenergy.BusItem.SetValue`
//!   calls.

pub mod subscriber;
pub mod writer;

pub use subscriber::Subscriber;
pub use writer::Writer;
