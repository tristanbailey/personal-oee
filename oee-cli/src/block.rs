//! `block …` commands: look at and correct the blocks of one day
//! (docs/CLI_API.md 5.2). They work on any date with a saved day, so a past
//! day can be back-filled.
//!
//! Blocks are addressed by start time: `10:40`, `10:40,11:00` or a range
//! `10:40..11:20` (every block starting in that span, ends included).

use std::collections::BTreeSet;

use clap::Subcommand;
use oee_core::{Block, BlockType, BulkChange, DayRecord, EditError, LostCategory, PlannedKind};
use serde_json::{json, Value};

use crate::day::{load_tracker, notices_json, now_for, open, parse_time, save, state_json, with_saved};
use crate::output::{CliError, Exit, Reply};
use crate::view::{block_json, category_name, hhmm};
use crate::Context;

#[derive(Subcommand, Debug)]
pub enum BlockCommand {
    /// Every block of the day with its type, category, Rework and note.
    List,
    /// Turn a block into work, lost time, a break or a meeting.
    SetType {
        #[arg(long, value_name = "HH:MM")]
        block: String,
        /// work | lost | break | meeting
        #[arg(long = "type", value_name = "TYPE")]
        kind: String,
    },
    /// Go back to what the day's events and calendar make the block.
    ResetType {
        #[arg(long, value_name = "HH:MM")]
        block: String,
    },
    /// Categorise lost blocks. All or none are changed.
    Categorise {
        /// Start times: HH:MM, comma list or HH:MM..HH:MM.
        #[arg(long, value_name = "BLOCKS")]
        blocks: String,
        /// system-down | change-over | idle | management | wait
        #[arg(long, value_name = "CATEGORY")]
        category: String,
    },
    /// Mark Work blocks as Rework, or unmark with --off. All or none are changed.
    Rework {
        #[arg(long, value_name = "BLOCKS")]
        blocks: String,
        #[arg(long)]
        off: bool,
    },
    /// Set a block's note; leave out --note to clear it.
    Note {
        #[arg(long, value_name = "HH:MM")]
        block: String,
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
    },
    /// One change to many blocks; blocks that can't take it are listed as skipped.
    Bulk {
        #[arg(long, value_name = "BLOCKS")]
        blocks: String,
        /// work | lost | break | meeting
        #[arg(long = "type", value_name = "TYPE")]
        kind: Option<String>,
        #[arg(long, value_name = "CATEGORY")]
        category: Option<String>,
        /// on | off
        #[arg(long, value_name = "on|off")]
        rework: Option<String>,
    },
    /// Lost blocks still waiting for a category.
    Uncategorised,
}

impl BlockCommand {
    pub fn name(&self) -> &'static str {
        match self {
            BlockCommand::List => "block list",
            BlockCommand::SetType { .. } => "block set-type",
            BlockCommand::ResetType { .. } => "block reset-type",
            BlockCommand::Categorise { .. } => "block categorise",
            BlockCommand::Rework { .. } => "block rework",
            BlockCommand::Note { .. } => "block note",
            BlockCommand::Bulk { .. } => "block bulk",
            BlockCommand::Uncategorised => "block uncategorised",
        }
    }
}

pub fn run(ctx: &Context, cmd: &BlockCommand) -> Result<Reply, CliError> {
    match cmd {
        BlockCommand::List => list(ctx),
        BlockCommand::Uncategorised => uncategorised(ctx),
        BlockCommand::SetType { block, kind } => {
            let start = parse_time(block, "--block")?;
            let ty = parse_type(kind)?;
            edit(ctx, &[start], |r, s| r.set_type(s, Some(ty)))
        }
        BlockCommand::ResetType { block } => {
            let start = parse_time(block, "--block")?;
            edit(ctx, &[start], |r, s| r.set_type(s, None))
        }
        BlockCommand::Categorise { blocks, category } => {
            let category = parse_category(category)?;
            edit_spec(ctx, blocks, |r, s| r.categorise(s, category))
        }
        BlockCommand::Rework { blocks, off } => {
            let on = !*off;
            edit_spec(ctx, blocks, |r, s| r.set_rework(s, on))
        }
        BlockCommand::Note { block, note } => {
            let start = parse_time(block, "--block")?;
            let note = note.clone();
            edit(ctx, &[start], |r, s| r.set_note(s, note.clone()))
        }
        BlockCommand::Bulk { blocks, kind, category, rework } => {
            let change = bulk_change(kind.as_deref(), category.as_deref(), rework.as_deref())?;
            bulk(ctx, blocks, change)
        }
    }
}

// ---- parsing ------------------------------------------------------------

fn parse_type(text: &str) -> Result<BlockType, CliError> {
    match text.trim().to_ascii_lowercase().as_str() {
        "work" => Ok(BlockType::Work),
        "lost" => Ok(BlockType::Lost),
        "break" => Ok(BlockType::Planned(PlannedKind::Break)),
        "meeting" => Ok(BlockType::Planned(PlannedKind::Meeting)),
        _ => Err(CliError::usage("bad_type", format!("Unknown block type \"{text}\"."))
            .hint("Use work, lost, break or meeting.")),
    }
}

fn parse_category(text: &str) -> Result<LostCategory, CliError> {
    let want = text.trim().to_ascii_lowercase();
    LostCategory::ALL
        .iter()
        .copied()
        .find(|c| category_name(*c) == want)
        .ok_or_else(|| {
            let all: Vec<&str> = LostCategory::ALL.iter().map(|c| category_name(*c)).collect();
            CliError::usage("bad_category", format!("Unknown category \"{text}\"."))
                .hint(format!("Use one of: {}.", all.join(", ")))
        })
}

fn bulk_change(kind: Option<&str>, category: Option<&str>, rework: Option<&str>) -> Result<BulkChange, CliError> {
    match (kind, category, rework) {
        (Some(k), None, None) => Ok(BulkChange::Type(parse_type(k)?)),
        (None, Some(c), None) => Ok(BulkChange::Category(parse_category(c)?)),
        (None, None, Some(r)) => match r.trim().to_ascii_lowercase().as_str() {
            "on" => Ok(BulkChange::Rework(true)),
            "off" => Ok(BulkChange::Rework(false)),
            _ => Err(CliError::usage("bad_rework", format!("Couldn't read --rework \"{r}\". Use on or off."))),
        },
        _ => Err(CliError::usage("bulk_needs_one_change", "Give exactly one of --type, --category or --rework.")),
    }
}

/// The starts a `--blocks` value names. A range expands to the blocks that
/// exist in it; single times are kept as written, so a wrong one is reported.
fn parse_spec(spec: &str, record: &DayRecord) -> Result<Vec<u32>, CliError> {
    let mut starts = BTreeSet::new();
    for part in spec.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once("..") {
            Some((a, b)) => {
                let (a, b) = (parse_time(a, "--blocks")?, parse_time(b, "--blocks")?);
                if a > b {
                    return Err(CliError::usage("bad_range", format!("The range \"{part}\" ends before it starts.")));
                }
                let inside: Vec<u32> = record
                    .blocks()
                    .iter()
                    .map(|bl| bl.start)
                    .filter(|s| (a..=b).contains(s))
                    .collect();
                if inside.is_empty() {
                    return Err(no_block(a));
                }
                starts.extend(inside);
            }
            None => {
                starts.insert(parse_time(part, "--blocks")?);
            }
        }
    }
    if starts.is_empty() {
        return Err(CliError::usage("no_blocks", "--blocks names no block.").hint("Use HH:MM, a comma list or HH:MM..HH:MM."));
    }
    Ok(starts.into_iter().collect())
}

// ---- errors ----------------------------------------------------------------

fn no_block(start: u32) -> CliError {
    CliError::new(Exit::NotFound, "no_such_block", format!("No block starts at {}.", hhmm(start)))
        .hint("Run `office-oee block list` and use a block's `start`.")
}

fn edit_error(e: EditError, start: u32) -> CliError {
    match e {
        EditError::NoSuchBlock => no_block(start),
        EditError::NotLostTime => CliError::new(
            Exit::Rule,
            "not_lost_time",
            format!("The block at {} is not lost time; only lost-time blocks can be categorised.", hhmm(start)),
        )
        .hint("Run `office-oee block list` and pick a block with kind \"lost\"."),
        EditError::NotWork => CliError::new(
            Exit::Rule,
            "not_work",
            format!("The block at {} is not Work; only Work blocks can be marked as Rework.", hhmm(start)),
        )
        .hint("Run `office-oee block list` and pick a block with kind \"work\"."),
    }
}

fn edit_code(e: EditError) -> &'static str {
    match e {
        EditError::NoSuchBlock => "no_such_block",
        EditError::NotLostTime => "not_lost_time",
        EditError::NotWork => "not_work",
    }
}

// ---- commands --------------------------------------------------------------

fn blocks_json(blocks: &[Block], only: Option<&BTreeSet<u32>>) -> Vec<Value> {
    blocks
        .iter()
        .filter(|b| only.map_or(true, |o| o.contains(&b.start)))
        .map(block_json)
        .collect()
}

fn list(ctx: &Context) -> Result<Reply, CliError> {
    let o = open(ctx, false)?;
    let (t, notices) = load_tracker(ctx, &o)?;
    let now = now_for(ctx, ctx.date);
    let live = t.live_record(now);
    let blocks: Vec<Value> = live
        .blocks()
        .iter()
        .map(|b| {
            let mut v = block_json(b);
            v["finished"] = json!(b.end <= now);
            v
        })
        .collect();
    Ok(Reply { result: json!({ "date": ctx.date.to_string(), "blocks": blocks }), notices: notices_json(&notices) })
}

fn uncategorised(ctx: &Context) -> Result<Reply, CliError> {
    let o = open(ctx, false)?;
    let (t, notices) = load_tracker(ctx, &o)?;
    let live = t.live_record(now_for(ctx, ctx.date));
    let waiting: BTreeSet<u32> = live.uncategorised().into_iter().collect();
    let blocks = blocks_json(&live.blocks(), Some(&waiting));
    Ok(Reply {
        result: json!({ "date": ctx.date.to_string(), "count": blocks.len(), "blocks": blocks }),
        notices: notices_json(&notices),
    })
}

/// Apply `change` to each start, all or nothing: the first refusal is the
/// error and nothing is saved.
fn edit(
    ctx: &Context,
    starts: &[u32],
    change: impl Fn(&mut DayRecord, u32) -> Result<(), EditError>,
) -> Result<Reply, CliError> {
    let o = open(ctx, true)?;
    let (mut t, notices) = load_tracker(ctx, &o)?;
    apply_strict(&mut t.record, starts, change)?;
    finish(ctx, &o, t, starts, notices)
}

fn edit_spec(
    ctx: &Context,
    spec: &str,
    change: impl Fn(&mut DayRecord, u32) -> Result<(), EditError>,
) -> Result<Reply, CliError> {
    let o = open(ctx, true)?;
    let (mut t, notices) = load_tracker(ctx, &o)?;
    let starts = parse_spec(spec, &t.record)?;
    apply_strict(&mut t.record, &starts, change)?;
    finish(ctx, &o, t, &starts, notices)
}

fn apply_strict(
    record: &mut DayRecord,
    starts: &[u32],
    change: impl Fn(&mut DayRecord, u32) -> Result<(), EditError>,
) -> Result<(), CliError> {
    // Work on a copy so a refusal part-way leaves the record untouched.
    let mut copy = record.clone();
    for &s in starts {
        change(&mut copy, s).map_err(|e| edit_error(e, s))?;
    }
    *record = copy;
    Ok(())
}

fn finish(
    ctx: &Context,
    o: &crate::day::Open,
    t: oee_core::Tracker,
    starts: &[u32],
    notices: Vec<oee_core::Notice>,
) -> Result<Reply, CliError> {
    let saved = save(ctx, o, &t)?;
    let only: BTreeSet<u32> = starts.iter().copied().collect();
    let mut state = with_saved(state_json(ctx, &t), saved);
    state["changed"] = json!(blocks_json(&t.record.blocks(), Some(&only)));
    Ok(Reply { result: state, notices: notices_json(&notices) })
}

fn bulk(ctx: &Context, spec: &str, change: BulkChange) -> Result<Reply, CliError> {
    let o = open(ctx, true)?;
    let (mut t, notices) = load_tracker(ctx, &o)?;
    let starts = parse_spec(spec, &t.record)?;
    // Why each refused block was skipped: try each on a copy first.
    let mut skipped = Vec::new();
    for &s in &starts {
        let mut copy = t.record.clone();
        let tried = match change {
            BulkChange::Type(ty) => copy.set_type(s, Some(ty)),
            BulkChange::Category(c) => copy.categorise(s, c),
            BulkChange::Rework(r) => copy.set_rework(s, r),
        };
        if let Err(e) = tried {
            skipped.push(json!({ "start": hhmm(s), "code": edit_code(e), "reason": e.to_string() }));
        }
    }
    let outcome = t.record.apply_bulk(&starts, change);
    debug_assert_eq!(outcome.skipped as usize, skipped.len());
    let saved = save(ctx, &o, &t)?;
    let mut state = with_saved(state_json(ctx, &t), saved);
    state["applied"] = json!(outcome.applied);
    state["skipped"] = json!(skipped);
    Ok(Reply { result: state, notices: notices_json(&notices) })
}

