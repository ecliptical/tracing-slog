//! Adapters for connecting structured log records from the `slog` crate into the `tracing` ecosystem.

use lazy_static::lazy_static;

use tracing_core::{
    callsite, dispatcher, field, identify_callsite,
    metadata::{Kind, Level},
    subscriber, Event, Metadata,
};

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
}

static FIELD_NAMES: &[&str] = &[
    "message",
    "slog.target",
    "slog.module",
    "slog.file",
    "slog.line",
    "slog.column",
];

impl Fields {
    fn new(cs: &'static dyn callsite::Callsite) -> Self {
        let fieldset = cs.metadata().fields();
        let message = fieldset.field("message").unwrap();
        let target = fieldset.field("slog.target").unwrap();
        let module = fieldset.field("slog.module").unwrap();
        let file = fieldset.field("slog.file").unwrap();
        let line = fieldset.field("slog.line").unwrap();
        let column = fieldset.field("slog.column").unwrap();
        Fields {
            message,
            target,
            module,
            file,
            line,
            column,
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

lazy_static! {
    static ref TRACE_FIELDS: Fields = Fields::new(&TRACE_CS);
    static ref DEBUG_FIELDS: Fields = Fields::new(&DEBUG_CS);
    static ref INFO_FIELDS: Fields = Fields::new(&INFO_CS);
    static ref WARN_FIELDS: Fields = Fields::new(&WARN_CS);
    static ref ERROR_FIELDS: Fields = Fields::new(&ERROR_CS);
}

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
