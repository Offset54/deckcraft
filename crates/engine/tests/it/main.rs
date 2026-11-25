// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md).
mod chart_native;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod contact_sheet;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod golden;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod pptx_roundtrip;
mod show_clicks;
