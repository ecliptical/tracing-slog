//! Adapters for connecting structured log records from the [slog] crate into the [tracing](https://github.com/tokio-rs/tracing) ecosystem.
//!
//! Use when a library uses `slog` but your application uses `tracing`.
//!
//! Heavily inspired by [tracing-log](https://github.com/tokio-rs/tracing/tree/5fdbcbf61da27ec3e600678121d8c00d2b9b5cb1/tracing-log).

use once_cell::sync::Lazy;

#[cfg(feature = "kv")]
use slog::KV;
use tracing_core::{
    callsite, dispatcher, field, identify_callsite,
    metadata::{Kind, Level},
    subscriber, Event, Metadata,
};

#[cfg(feature = "kv")]
/// An allocating serializer to use for serializing key-value pairs in a [`slog::Record`]
#[derive(Default)]
struct TracingKvSerializer {
    storage: String,
}

#[cfg(feature = "kv")]
impl TracingKvSerializer {
    /// Returns the serialized fields as a string. If empty, returns `None`.
    fn as_str(&self) -> Option<&str> {
        self.storage
            .get(..self.storage.len().saturating_sub(1))
            .filter(|kv_serialization| !kv_serialization.is_empty())
    }
}

#[cfg(feature = "kv")]
impl slog::Serializer for TracingKvSerializer {
    fn emit_arguments(&mut self, key: slog::Key, val: &core::fmt::Arguments) -> slog::Result {
        self.storage.push_str(&format!("{key}={val},"));
        Ok(())
    }
}

/// A [slog Drain](slog::Drain) that converts [records](slog::Record) into [tracing events](Event).
///
/// To use, create a [slog logger](slog::Logger) using an instance of [TracingSlogDrain] as its drain:
///
/// ```rust
/// # use slog::*;
/// # use tracing_slog::TracingSlogDrain;
/// let drain = TracingSlogDrain;
/// let root = Logger::root(drain, o!());
///
/// info!(root, "logged using slogger");
/// ```
#[derive(Debug)]
pub struct TracingSlogDrain;

impl slog::Drain for TracingSlogDrain {
    type Ok = ();
    type Err = slog::Never;

    /// Converts a [slog record](slog::Record) into a [tracing event](Event)
    /// and dispatches it to any registered tracing subscribers
    /// using the [default dispatcher](dispatcher::get_default).
    /// Currently, the key-value pairs are ignored.
    fn log(
        &self,
        record: &slog::Record<'_>,
        _values: &slog::OwnedKVList,
    ) -> Result<Self::Ok, Self::Err> {
        dispatcher::get_default(|dispatch| {
            let filter_meta = slogrecord_to_trace(record);
            if !dispatch.enabled(&filter_meta) {
                return;
            }

            #[cfg(feature = "kv")]
            let kv_serializer = {
                let mut ser = TracingKvSerializer::default();
                let _ = record.kv().serialize(record, &mut ser);
                ser
            };

            let (_, keys, meta) = sloglevel_to_cs(record.level());

            let target = get_target(record);

            dispatch.event(&Event::new(
                meta,
                &meta.fields().value_set(&[
                    (&keys.message, Some(record.msg() as &dyn field::Value)),
                    (&keys.target, Some(&target)),
                    (&keys.module, Some(&record.module())),
                    (&keys.file, Some(&record.file())),
                    (&keys.line, Some(&record.line())),
                    (&keys.column, Some(&record.column())),
                    #[cfg(feature = "kv")]
                    (
                        &keys.kv,
                        kv_serializer
                            .as_str()
                            .as_ref()
                            .map(|x| x as &dyn tracing_core::field::Value),
                    ),
                ]),
            ));
        });

        Ok(())
    }
}

fn get_target<'a>(record: &'a slog::Record<'a>) -> &'a str {
    let target = record.tag();
    if target.is_empty() {
        record.module()
    } else {
        target
    }
}

struct Fields {
    message: field::Field,
    target: field::Field,
    module: field::Field,
    file: field::Field,
    line: field::Field,
    column: field::Field,
    #[cfg(feature = "kv")]
    kv: field::Field,
}

static FIELD_NAMES: &[&str] = &[
    "message",
    "slog.target",
    "slog.module_path",
    "slog.file",
    "slog.line",
    "slog.column",
    #[cfg(feature = "kv")]
    "slog.kv",
];

impl Fields {
    fn new(cs: &'static dyn callsite::Callsite) -> Self {
        let fieldset = cs.metadata().fields();
        let message = fieldset.field("message").unwrap();
        let target = fieldset.field("slog.target").unwrap();
        let module = fieldset.field("slog.module_path").unwrap();
        let file = fieldset.field("slog.file").unwrap();
        let line = fieldset.field("slog.line").unwrap();
        let column = fieldset.field("slog.column").unwrap();
        #[cfg(feature = "kv")]
        let kv = fieldset.field("slog.kv").unwrap();
        Fields {
            message,
            target,
            module,
            file,
            line,
            column,
            #[cfg(feature = "kv")]
            kv,
        }
    }
}

macro_rules! slog_cs {
    ($level:expr, $cs:ident, $meta:ident, $ty:ident) => {
        struct $ty;
        static $cs: $ty = $ty;
        static $meta: Metadata<'static> = Metadata::new(
            "slog event",
            "slog",
            $level,
            None,
            None,
            None,
            field::FieldSet::new(FIELD_NAMES, identify_callsite!(&$cs)),
            Kind::EVENT,
        );

        impl callsite::Callsite for $ty {
            fn set_interest(&self, _: subscriber::Interest) {}
            fn metadata(&self) -> &'static Metadata<'static> {
                &$meta
            }
        }
    };
}

slog_cs!(
    tracing_core::Level::TRACE,
    TRACE_CS,
    TRACE_META,
    TraceCallsite
);

slog_cs!(
    tracing_core::Level::DEBUG,
    DEBUG_CS,
    DEBUG_META,
    DebugCallsite
);

slog_cs!(tracing_core::Level::INFO, INFO_CS, INFO_META, InfoCallsite);

slog_cs!(tracing_core::Level::WARN, WARN_CS, WARN_META, WarnCallsite);

slog_cs!(
    tracing_core::Level::ERROR,
    ERROR_CS,
    ERROR_META,
    ErrorCallsite
);

static TRACE_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&TRACE_CS));
static DEBUG_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&DEBUG_CS));
static INFO_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&INFO_CS));
static WARN_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&WARN_CS));
static ERROR_FIELDS: Lazy<Fields> = Lazy::new(|| Fields::new(&ERROR_CS));

fn sloglevel_to_cs(
    level: slog::Level,
) -> (
    &'static dyn callsite::Callsite,
    &'static Fields,
    &'static Metadata<'static>,
) {
    match level {
        slog::Level::Trace => (&TRACE_CS, &*TRACE_FIELDS, &TRACE_META),
        slog::Level::Debug => (&DEBUG_CS, &*DEBUG_FIELDS, &DEBUG_META),
        slog::Level::Info => (&INFO_CS, &*INFO_FIELDS, &INFO_META),
        slog::Level::Warning => (&WARN_CS, &*WARN_FIELDS, &WARN_META),
        slog::Level::Error | slog::Level::Critical => (&ERROR_CS, &*ERROR_FIELDS, &ERROR_META),
    }
}

fn sloglevel_to_trace(level: slog::Level) -> Level {
    match level {
        slog::Level::Trace => Level::TRACE,
        slog::Level::Debug => Level::DEBUG,
        slog::Level::Info => Level::INFO,
        slog::Level::Warning => Level::WARN,
        slog::Level::Error | slog::Level::Critical => Level::ERROR,
    }
}

fn slogrecord_to_trace<'a>(record: &'a slog::Record<'a>) -> Metadata<'a> {
    let cs_id = identify_callsite!(sloglevel_to_cs(record.level()).0);
    let target = get_target(record);

    Metadata::new(
        "slog record",
        target,
        sloglevel_to_trace(record.level()),
        Some(record.file()),
        Some(record.line()),
        Some(record.module()),
        field::FieldSet::new(FIELD_NAMES, cs_id),
        Kind::EVENT,
    )
}

#[cfg(test)]
mod tests {
    use super::TracingSlogDrain;
    use slog::*;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn basic() {
        let drain = TracingSlogDrain;
        let root = Logger::root(drain, o!());

        info!(root, "slog test"; "arg1" => "val1");
        assert!(logs_contain("slog test"));
    }

    #[cfg(feature = "kv")]
    #[test]
    #[traced_test]
    fn key_value_pairs() {
        let drain = TracingSlogDrain;
        let root = Logger::root(drain, o!());

        info!(root, "slog test"; "arg1"=>"val1", "arg2"=>"val2");
        assert!(logs_contain("slog test"));
        assert!(
            logs_contain("arg1=val1"),
            "first kv pair should be included"
        );
        assert!(
            logs_contain("arg2=val2"),
            "second kv pair should be included"
        );
        assert!(
            logs_contain("arg2=val2,arg1=val1"),
            "comma-separated kv pairs should be included"
        );
        assert!(
            !logs_contain("arg1=val1,arg1=val1,"),
            "trailing comma should not be included"
        );
    }

    #[cfg(feature = "kv")]
    #[test]
    #[traced_test]
    fn non_string_key_value_pairs() {
        let drain = TracingSlogDrain;
        let root = Logger::root(drain, o!());

        info!(root, "slog test"; "log-key" => true);
        assert!(
            logs_contain("log-key=true"),
            "first kv pair should be included"
        );

        #[allow(unused)]
        #[derive(Debug)]
        struct Wrapper(u8);

        let w = Wrapper(100);

        info!(root, "slog test"; "debug-struct" =>?w);
        assert!(
            logs_contain("debug-struct=Wrapper(100)"),
            "Debug-formatted struct should be included"
        );
    }

    #[cfg(feature = "kv")]
    #[test]
    #[traced_test]
    fn log_without_kv_pair_doesnt_contain_kv_field() {
        let drain = TracingSlogDrain;
        let root = Logger::root(drain, o!());

        info!(root, "slog test");
        assert!(
            !logs_contain("slog.kv"),
            "log without key-value pair should not contain `slog.kv`"
        );
    }

    mod nested_mod {
        pub fn log_as_info(slogger: &slog::Logger) {
            slog::info!(slogger, "slog test");
        }
    }

    #[test]
    #[traced_test]
    fn nested() {
        let drain = TracingSlogDrain;
        let root = Logger::root(drain, o!());

        nested_mod::log_as_info(&root);
        assert!(logs_contain("nested_mod"));
    }
}
