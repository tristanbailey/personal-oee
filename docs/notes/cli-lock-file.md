# Note: the CLI lock file, now or later?

Reference note for the CLI work (see `docs/CLI_API.md`, section 6, and item C-11).

## The problem

The desktop app reads today's day file once, at launch or on rolling over to a new date. After that it keeps the day in memory and writes it back only when something in the app changes. It never checks whether the file changed on disk.

So if the app is open and the CLI edits today's record (for example marks a block as rework), the app's next change writes its in-memory copy over the file and silently removes the CLI's edit. The reverse can also happen. Neither side gets a warning.

## What the lock file does

While the app runs it holds a marker file, `<data-dir>/.lock`. A CLI command that writes data finds the marker and stops with exit code 6 (`app_running`) instead of risking the overwrite. Reads are always allowed. `--force` overrides it.

It protects data by making the clash visible. It does not make CLI edits to live data work.

## Why it is a "now or later" choice

It only matters if CLI writes run against the real data folder while the app is open. The main use case, an LLM testing against a throwaway folder with `--data-dir`, never involves the app, so the lock does nothing there. Since writes need an explicit flag to choose real or test data, the risky case already takes a deliberate step.

## Options

- **Later (recommended).** Build C-01 to C-10 first. Until then: use a test folder, or close the app before writing to the real folder.
- **Now.** Add the lock with the CLI (C-11). It is small but touches app startup and shutdown, and a crash can leave a stale lock, so it needs a check that the owning process is still alive.
- **Fuller fix, later still.** Have the app reload today's file when it changes on disk. This is what would make CLI edits to live data actually work. It is a separate item.

## Decision

Open. Recommendation: later. Nothing in C-01 to C-10 depends on the lock.
