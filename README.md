# Sweep

A small Mac utility in Rust (Tauri 2 + Svelte) with three screens:

- **Cleanup**: finds caches, logs and other leftovers, and moves the ones you pick to the Trash.
- **Disk Map**: a size tree and treemap of a folder, plus its largest files, so you can clean
  folder by folder.
- **Health**: CPU, memory, fans and top processes. Read-only.

Design: https://claude.ai/artifact/QWtuEADaUQvKrmnC8odh8F

## Run

Needs Rust (rustup), Node and pnpm. Tauri's CLI comes in as a dev dependency.

```bash
pnpm install
pnpm tauri dev      # dev app with hot reload
pnpm tauri build    # Sweep.app and a .dmg in src-tauri/target/release/bundle
```

If `pnpm tauri dev` says it can't run `cargo metadata`, your shell is missing `~/.cargo/bin` on its
PATH. Add `. "$HOME/.cargo/env"` to `~/.zshrc`, or run it once as
`PATH="$HOME/.cargo/bin:$PATH" pnpm tauri dev`.

Checks:

```bash
cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings
pnpm check
cd src-tauri && cargo run --example probe   # print one health sample and the cleanup scan
```

## What Cleanup touches

The items it lists are always the **direct children** of the folders below, all under your home
folder. The folders themselves stay.

| Group | Folders | Default |
|---|---|---|
| System & app caches | `~/Library/Caches` (browser folders excluded) | selected |
| Logs & crash reports | `~/Library/Logs`, items unused for 7+ days | selected |
| Developer caches | Xcode `DerivedData`, `CoreSimulator/Caches`, `~/.cargo/registry/cache`, `~/.npm/_cacache`, `~/Library/pnpm/store` | selected |
| Browser caches | Safari, Chrome, Arc and Firefox cache folders, never profiles or cookies | selected |
| Trash | `~/.Trash`, shown for size only | never cleaned |
| Old downloads | `~/Downloads`, items unused for 90+ days | review, not selected |
| Docker, Homebrew | planned: `docker system prune`, `brew cleanup` | coming later |

Cache folders named after a running app's bundle id are skipped while that app is open.

## Safety

- Nothing is deleted permanently. Everything goes through the macOS Trash (`trash` crate), so you
  can put it back.
- Every path passes `safety::check` first. It must:
  - be absolute, with no `..`
  - not be reached through a symlinked folder
  - be inside your home folder and inside a scanned root
  - not be a scanned root, your home folder, or a standard folder such as `~/Library` or
    `~/Documents`
- Cleanup only trashes paths its last scan listed. Disk Map only trashes paths inside the folder
  it scanned.
- Health only reads. The SMC code uses the read selectors and never writes keys, so fan speeds are
  never changed.

## Full Disk Access

macOS protects some folders, such as Safari's caches, `~/.Trash` and parts of `~/Library`. Sweep
can't read them until you allow it under **System Settings › Privacy & Security › Full Disk
Access**. Until then, both screens show how many folders they skipped.

## Known limits

- "Last used" is the later of a file's access and modification times. Spotlight's last-opened
  date would be more accurate.
- CPU temperature reads undocumented SMC keys. It's best effort and shows "Unavailable" when no key
  answers.
- Sizes are space actually used on disk (allocated blocks), with hard links counted once, so they
  can differ from Finder's "size" for sparse files.

## Layout

```
src/                     Svelte front end
  lib/api.ts             typed invoke() wrappers and events
  lib/screens/           Cleanup, DiskMap, Health
  lib/components/        Sidebar, Treemap, Sparkline, SizeBar, ConfirmDialog, ...
src-tauri/src/
  commands.rs            Tauri commands (thin)
  safety.rs              path guard + move to Trash
  fsx.rs                 size on disk, volume info
  cleanup/               rules, scan, clean
  disk/                  parallel walk, size tree, largest files
  health/                cpu, memory, smc (fans, temperature), processes, sysctl
```
