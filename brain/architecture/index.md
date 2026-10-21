# Architecture

The technical layers and cross-cutting subsystems. Start with
[overview.md](overview.md) for the big picture, then jump to the layer you need.
Back to the root map: [../BRAIN.md](../BRAIN.md).

| File | What's here | Grep keywords |
|---|---|---|
| [overview.md](overview.md) | Workspace, data flow from the CLIs to four surfaces, the three clocks, what lives in core / binary / frontend, layout rules | crates, gather, panel --json, watch, poll, settle, attentive |
| [panel-spec.md](panel-spec.md) | `panel_spec`, the JSON shape, the seven sections, row fields, `--ui` and `gather::Ui`, English-only strings, `parity.rs` and `specification.rs` | Panel, Section, Row, SECTION_IDS, navigation, searchKey, copyOptions, busy, payload, PanelState, ResolveOptions |
| [frontends.md](frontends.md) | Plasma, GNOME, Omarchy and the TUI: services, clicks on the bar icon, settings storage, GNOME 45/46/48 differences, IPC, logs | ProviderService.qml, provider.ts, extension.ts, Service.qml, Panel.qml, PopupMenu, vadjustment, omarchy-shell |
| [providers.md](providers.md) | The registry, capabilities, each provider's argv, detection and the active provider, the parsers and fixtures, gathering, `tailgauge watch` | PROVIDERS, ProviderDescriptor, Capability, status.rs, netbird.rs, exit_nodes.rs, accounts.rs, watch-ipn |
| [binary.md](binary.md) | The command line and exit codes, argv[0] alias dispatch, `ctl` attended / unattended, `notify`, `copy`, `file-select` | clap, splice_alias, ALIASES, ctl, attended, notify-send, wl-copy, zenity, kdialog |
| [testing.md](testing.md) | Specification, parity, e2e against fake CLIs, the QML and GNOME node harnesses, the real headless GNOME Shell | specification.rs, parity.rs, e2e.rs, tests/qml, tests/gnome, run.sh, fixtures |
| [distribution.md](distribution.md) | Binary names, the selvedge `Project`, `install.sh`, release assets and their tests, `.deb`/`.rpm`/Arch packages and signing, stores, `--install-frontend`, the 0.4.0 bootstrap, licence | project.rs, selvedge, arch_target, frontends/, install.sh, SHA256SUMS, nfpm, PKGBUILD, musl, helpers.tar.gz, kpackage, EGO, NOTICE |
