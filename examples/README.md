# Example data

`week/` is a ready-made data folder for testing: one working week (Mon 2 – Fri 6 Nov 2026) with a lunch break every day, a Tuesday meeting, lost time in several categories with notes, Rework, a holiday on Wednesday, a long Thursday and a short Friday. The week scores 84.9% OEE, just under the default 85% goal.

| Day | What it shows |
|---|---|
| Mon 2 Nov | Wait, Change Over and Management stops with notes; one Rework block |
| Tue 3 Nov | Break and meeting from the calendar; System Down for two blocks (VPN); two Rework blocks |
| Wed 4 Nov | Holiday: no record |
| Thu 5 Nov | Day extended to 19:00 (higher target); over capacity |
| Fri 6 Nov | Day ended at 15:00 (lower target); over capacity |

## Open it in the app

```sh
OEE_DATA_DIR="$PWD/examples/week" dist/PersonalOEE.app/Contents/MacOS/PersonalOEE
```

Open the Week tab and go to the week of 2 Nov 2026 (Back arrow), or click a day. The folder is a copy: change it freely, then restore it with the command below.

## Use it with the command-line tool

```sh
office-oee --data-dir examples/week --date 2026-11-04 report week --text
office-oee --data-dir examples/week check
```

## Regenerate it

```sh
office-oee --data-dir examples/week --date 2026-11-04 --yes data seed --scenario example-week
```

A test (`the_committed_example_week_is_healthy_and_matches_the_seed`) fails if this folder no longer matches the `example-week` scenario in `oee-store/src/seed.rs`.
