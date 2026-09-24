# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.3.0] - 2026-09-24
### Bug Fixes

- Add version headers to cliff template; skip release/changelog commits

### Documentation

- Update for v0.2.0
- Token-handling rules for agents + privacy warning on token-in-prompt
- Token-shy flow follows plugin convention — .env in state dir, then register

### Features

- --yes on a TTY prompts for the token only
## [0.2.0] - 2026-09-24
### Bug Fixes

- Keep target filename so sha256sum -c can verify

### Documentation

- Update for v0.1.0
- Add fresh-setup and migration guides; register generates run.sh
- Fold example asks into the copyable agent block
- Split agent examples into per-case copyable blocks
- Move agent-first guide to top of Guides

### Features

- Non-interactive mode for agent-driven onboarding
- `dcbot agent.md` prints the agent usage contract

### Rename

- Dcbot agents.md as canonical (agent.md kept as alias), AGENTS.md file
## [0.1.0] - 2026-09-24
### Bug Fixes

- Executable bit on install.sh, changelog job pushes HEAD:main
- Macos-13 runner retired — use macos-15-intel for x86_64 builds

### Features

- Scaffold dcbot — multi-bot manager for discord@claude-plugins-official

### Miscellaneous

- Tune crates.io keywords
- Point repository metadata at tidusvn05/dcbot

