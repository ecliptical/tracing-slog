# tracing-slog

Adapters for connecting structured log records from the [`slog`](https://github.com/slog-rs/slog) crate
into the [`tracing`](https://github.com/tokio-rs/tracing) ecosystem.

Use when a library uses `slog` but your application uses `tracing`.

Heavily inspired by [tracing-log](https://github.com/tokio-rs/tracing/tree/5fdbcbf61da27ec3e600678121d8c00d2b9b5cb1/tracing-log).
