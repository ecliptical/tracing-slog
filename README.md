# tracing-slog

Adapters for connecting structured log records from the [`slog`](https://github.com/slog-rs/slog) crate
into the [`tracing`](https://github.com/tokio-rs/tracing) ecosystem.

Use when a library uses `slog` but your application uses `tracing`.

Heavily inspired by [tracing-log](https://github.com/tokio-rs/tracing/tree/60c60bef62972e447414a748a95b31ff9027165b/tracing-log).

Specifically, the emitted log entries include the custom fields `slog.target`, `slog.module_path`, `slog.file`, `slog.line`, and `slog.column` with corresponding values from the slog call site.

Note that the "native" `filename` and `line_number` metadata attributes will never be available (and `target` will always be `slog`). This is due to the fact that `tracing` requires static metadata constructed at the original call site. The `tracing-log` adapter does provide these due to explicit support in `tracing`.
