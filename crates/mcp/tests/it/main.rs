// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md).
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod automation_roots;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod batch_budget;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod phase1_sample;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod phase2_sample;
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod phase3_sample;
