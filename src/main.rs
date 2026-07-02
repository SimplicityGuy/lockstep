mod cli;
mod context;
mod registry;
mod report;
mod updaters;
mod version;

use std::io::Write;

use clap::{CommandFactory, Parser};

use cli::{Cli, Command, RunArgs};
use context::{Ecosystem, RunCtx, Runner, SystemRunner};
use report::{EcoLog, OptionsLog, Reporter, RunLog, Status, SummaryLog, write_json_log};

fn main() {
    let cli = Cli::parse();
    let code = match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("❌ clockpin: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command.unwrap_or(Command::Run(RunArgs::default())) {
        Command::Run(args) => orchestrate(args, false),
        Command::Check(args) => orchestrate(args, true),
        Command::List(p) => list(p.path.as_deref()),
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "clockpin", &mut std::io::stdout());
            Ok(0)
        }
    }
}

fn orchestrate(args: RunArgs, force_dry_run: bool) -> anyhow::Result<i32> {
    let quiet = args.quiet;
    let verbose = args.verbose;
    let color = !args.no_color && console::colors_enabled();
    let log_json = args.log_json.clone();
    let ctx: RunCtx = args.into_ctx(force_dry_run);

    let reporter = Reporter {
        quiet,
        verbose,
        color,
    };
    reporter.header();

    let all = updaters::all();
    // Detect (respecting selection) into jobs, so the "detected:" line can
    // print before running.
    let mut jobs: Vec<(&dyn updaters::Updater, Ecosystem, updaters::Detection)> = Vec::new();
    for up in &all {
        let eco = up.ecosystem();
        if !ctx.selected(eco) {
            continue;
        }
        let det = up.detect(&ctx.repo_root);
        jobs.push((up.as_ref(), eco, det));
    }
    let present: Vec<Ecosystem> = jobs
        .iter()
        .filter(|(_, _, d)| matches!(d, updaters::Detection::Present { .. }))
        .map(|(_, e, _)| *e)
        .collect();
    let absent: Vec<(Ecosystem, String)> = jobs
        .iter()
        .filter_map(|(_, e, d)| match d {
            updaters::Detection::Absent { reason } => Some((*e, reason.clone())),
            _ => None,
        })
        .collect();
    reporter.detected(&present, &absent);

    let mut eco_logs = Vec::new();
    let mut held_all = Vec::new();
    let mut updated = 0usize;
    let mut touched = 0usize;
    let mut errors = 0usize;

    for (up, eco, det) in &jobs {
        let out = up.run(&ctx, det);
        reporter.line(*eco, &out, ctx.is_frozen(*eco));
        if !out.changes.is_empty() {
            updated += out.changes.len();
            touched += 1;
        }
        if out.status == Status::Errored {
            errors += 1;
        }
        held_all.extend(out.held_back.iter().cloned());
        eco_logs.push(EcoLog {
            key: eco.key().to_string(),
            detected: matches!(det, updaters::Detection::Present { .. }),
            tool_available: out.status != Status::Warned,
            status: out.status.as_str().to_string(),
            changes: out.changes.clone(),
            warnings: out.warnings.clone(),
            errors: out.errors.clone(),
        });
    }

    reporter.held_back(&held_all);
    reporter.summary(updated, touched, errors);

    if let Some(path) = log_json {
        let log = RunLog {
            tool: "clockpin".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            repo_root: ctx.repo_root.to_string_lossy().into_owned(),
            options: OptionsLog {
                dry_run: ctx.dry_run,
                freeze: {
                    let mut freeze: Vec<String> =
                        ctx.freeze.iter().map(|e| e.key().to_string()).collect();
                    freeze.sort();
                    freeze
                },
                major: ctx.major,
                only: ctx.only.iter().map(|e| e.key().to_string()).collect(),
                skip: ctx.skip.iter().map(|e| e.key().to_string()).collect(),
            },
            ecosystems: eco_logs,
            held_back_majors: held_all,
            summary: SummaryLog {
                updated,
                ecosystems_touched: touched,
                errors,
            },
        };
        write_json_log(&path, &log)?;
    }

    Ok(if errors > 0 { 1 } else { 0 })
}

fn list(path: Option<&std::path::Path>) -> anyhow::Result<i32> {
    let root = cli::resolve_root(path);
    let tool_for = |e: Ecosystem| match e {
        Ecosystem::Uv | Ecosystem::Requirements => "uv",
        Ecosystem::Npm => "npx",
        Ecosystem::Cargo => "cargo",
        Ecosystem::Actions => "gh",
        Ecosystem::PreCommit => "pre-commit",
        Ecosystem::Docker => "(built-in)",
    };
    let runner = SystemRunner;
    let mut stdout = std::io::stdout();
    for up in updaters::all() {
        let eco = up.ecosystem();
        let present = matches!(up.detect(&root), updaters::Detection::Present { .. });
        let tool = tool_for(eco);
        let avail = tool == "(built-in)" || runner.which(tool);
        writeln!(
            stdout,
            "  {} {:<12} detected={} tool={} available={}",
            eco.emoji(),
            eco.key(),
            present,
            tool,
            avail
        )?;
    }
    Ok(0)
}
