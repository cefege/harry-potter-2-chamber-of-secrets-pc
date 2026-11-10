# Repository Rust skills

These are exact copies from [Morgan Linton's BeautifulRust](https://github.com/morganlinton/BeautifulRust), commit `b9718436d77ee7bcf17cdcb6aa27628feb95d97a`, under its MIT license. See `LICENSE.BeautifulRust` and `beautiful-rust.lock.json` for attribution, source paths, and SHA-256 hashes.

Use the local skill entry points:

- [`rust-best-practices/SKILL.md`](rust-best-practices/SKILL.md) when writing or changing Rust, Cargo manifests, tests, or rustdoc.
- [`rust-review/SKILL.md`](rust-review/SKILL.md) with best practices when reviewing Rust.
- [`rust-port/SKILL.md`](rust-port/SKILL.md) with best practices when preserving behavior during a port or structural migration.

Read only the reference digests needed for the task. Bevy guidance applies only if a crate actually uses Bevy. Evaluation corpora, plugins, hosted services, and a global installation are not required or vendored.

Repository rules and [`Docs/RUST_STYLE.md`](../../Docs/RUST_STYLE.md) take precedence where generic guidance conflicts with measured engine compatibility. In particular, do not apply unrestricted `--all-features`, workspace formatting rewrites, API redesigns, or dependency unpinning mechanically.

The files under `upstream/` are the reviewed upstream lint suggestions, **not active project configuration**. Lint enforcement must graduate against recorded package and feature baselines. Do not enable every suggested lint as an immediate workspace error or hide warning debt with broad suppressions.

Agents that discover `.agents/skills/*/SKILL.md` can load these directly. Other agents must follow the links in `AGENTS.md`. No agent must rely on the temporary upstream checkout.

Updates must be a separate dependency-style change: select a commit, review skill and reference changes, retain the license, update the lock hashes, validate local links, and run the contract tests. Do not silently edit the vendored upstream files to encode engine policy; keep adaptations in `Docs/RUST_STYLE.md`.
