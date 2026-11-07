# Warnings Backlog (v1.1)

Warnings discovered during v1.0.x builds are deferred here rather than fixed
randomly. Address in v1.1 under the relevant subsystem.

- Kernel: review any unused/dead-code warnings surfaced by `cargo build`.
- sdk: review warnings in package_cmd/run_cmd.
- apps: review warnings in userland crates.
- Do not fix warnings outside the subsystem being actively changed.
