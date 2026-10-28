# glaunch (fork with GUI)

Fork of [cebarks/glaunch](https://github.com/cebarks/glaunch) — a Steam game
launch wrapper with gamescope, gamemode, HDR, V-Cache, and per-game profiles —
plus a native desktop GUI for building launch commands.

## What's in this fork

Everything upstream has, plus:

- **`gui/`** — `glaunch-gui`, an [egui](https://github.com/emilk/egui) desktop
  app that lets you pick launch settings with checkboxes and copy the finished
  Steam Launch Options command. It can also save/load/delete profiles in
  `~/.config/glaunch/profiles/` using glaunch's own library (path dependency),
  so profile files always match the format the CLI reads.

## Layout

```
src/     glaunch CLI (unchanged from upstream)
tests/   upstream tests
gui/     glaunch-gui desktop app (this fork's addition)
```

The repo is a Cargo workspace: `cargo build` builds both binaries.

## Build

```sh
cargo build --release
```

- CLI: `target/release/glaunch`
- GUI: `target/release/glaunch-gui`

## Using the GUI

Run `glaunch-gui`. Left side: display, picture/system, and overlay settings.
Right side: the live `glaunch run … -- %command%` command to paste into
Steam → game → Properties → Launch Options, a dry-run preview that shells out
to the real `glaunch` binary (must be on `PATH`), and profile management.

## Keeping up with upstream

```sh
git remote add upstream https://github.com/cebarks/glaunch
git fetch upstream
git merge upstream/main
```

Only `Cargo.toml` (workspace line) and the new `gui/` folder differ from
upstream, so merges should stay clean.
