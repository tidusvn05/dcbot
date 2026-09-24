# Contributing

Thanks for your interest in dcbot.

## Development

```bash
cargo build          # debug build → target/debug/dcbot
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

Requires Rust stable. Runtime deps for integration testing: `tmux`, `claude` (Claude Code CLI), `bun`.

## Conventional commits

Commits must follow [Conventional Commits](https://www.conventionalcommits.org/) — the changelog
(`git-cliff`) is generated from them:

- `feat:` — new feature (appears under **Features**)
- `fix:` — bug fix (**Bug Fixes**)
- `docs:`, `perf:`, `refactor:`, `style:`, `test:`, `chore:`

## i18n

All user-facing strings go through `t!()` with keys in `locales/{en,vi,ja}.yml`.
When adding a string: add the key to **all three** files. English is the fallback.

## Releasing (maintainers)

```bash
git tag v0.x.y
git push origin v0.x.y
```

The release workflow builds Linux/macOS binaries, generates release notes with
`git-cliff`, publishes the GitHub Release, and commits the regenerated
`CHANGELOG.md` back to `main`.
