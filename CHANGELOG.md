# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
## [0.11.0] - 2026-10-12
### Bug Fixes

- Use load_style for comfy-table 8

### Features

- Lock default DM access and add self-update

### Miscellaneous

- Bump comfy-table from 7.2.2 to 8.0.1
- Bump dirs from 5.0.1 to 7.0.0
- Bump rust-i18n from 4.2.2 to 4.2.4
- Bump libc from 0.2.189 to 0.2.190

### Styling

- Cargo fmt

### Ci

- Match lint check name to branch rule

### Merge

- Restore v0.10.0 release commits onto main
## [0.10.0] - 2026-09-26
### Bug Fixes

- Detect Message Content Intent LIMITED flag

### Features

- Verify the bot is live before greeting, recycle dead sessions
## [0.9.0] - 2026-09-24
### Documentation

- Migrate by copying global state into the existing bot dir

### Features

- Auto-allow the standard toolset in new deployments
- Allow unrestricted Bash in generated deployments
## [0.8.0] - 2026-09-24
### Bug Fixes

- Make discord replies work in headless sessions
## [0.7.0] - 2026-09-24
### Features

- Pairing helpers + .claude/rules/dcbot.md per deployment

### Miscellaneous

- Bump cargo deps + github actions (dependabot #1-#9)
## [0.6.0] - 2026-09-24
### Documentation

- Say "follow cli `dcbot agents.md`" in agent-first examples

### Features

- DM a greeting to the owner when the session comes up
- Add `dcbot dm`; greet configured channels on start
## [0.5.0] - 2026-09-24
### Bug Fixes

- Pre-accept Claude Code folder trust so first launch doesn't hang

### Features

- --open/--copy flags + open-in-browser prompt in the wizard
- Auto-install discord channel plugin before launch
## [0.4.0] - 2026-09-24
### Bug Fixes

- Drop stray raw binary from dist; order cliff skip parsers before groups

### Documentation

- Keep Install minimal; move version-pinning and source build to the bottom

### Features

- Generate OAuth2 invite URL; add `dcbot invite` + intent checks
## [0.3.0] - 2026-09-24
### Bug Fixes

- Add version headers to cliff template; skip release/changelog commits

### Documentation

- Token-handling rules for agents + privacy warning on token-in-prompt
- Token-shy flow follows plugin convention — .env in state dir, then register

### Features

- --yes on a TTY prompts for the token only
## [0.2.0] - 2026-09-24
### Bug Fixes

- Keep target filename so sha256sum -c can verify

### Documentation

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

