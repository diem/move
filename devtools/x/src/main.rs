// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use std::{boxed::Box, io::Write};

use chrono::Local;
use clap::{Parser, Subcommand};
use env_logger::{self, fmt::Color};
use log::Level;

use crate::clippy::ClippyArgs;
use build::BuildArgs;
use changed_since::ChangedSinceArgs;
use check::CheckArgs;
use diff_summary::DiffSummaryArgs;
use fix::FixArgs;
use fmt::FmtArgs;
use generate_summaries::GenerateSummariesArgs;
use generate_workspace_hack::GenerateWorkspaceHackArgs;
use lint::LintArgs;
use nextest::NextestArgs;
use playground::PlaygroundArgs;
use test::TestArgs;
use tools::ToolsArgs;

mod bench;
mod build;
mod cargo;
mod changed_since;
mod check;
mod clippy;
mod config;
mod context;
mod diff_summary;
mod fix;
mod fmt;
mod generate_summaries;
mod generate_workspace_hack;
mod installer;
mod lint;
mod nextest;
mod playground;
mod test;
mod tools;
mod utils;

type Result<T> = anyhow::Result<T>;

#[derive(Debug, Parser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run `cargo bench`
    #[clap(name = "bench")]
    Bench(bench::BenchArgs),

    /// Run `cargo build`
    // the argument must be Boxed due to it's size and clippy (it's quite large by comparison to others.)
    #[clap(name = "build")]
    Build(Box<BuildArgs>),

    /// Run `cargo check`
    #[clap(name = "check")]
    Check(CheckArgs),

    /// List packages changed since merge base with the given commit
    ///
    /// Note that this compares against the merge base (common ancestor) of the specified commit.
    /// For example, if origin/master is specified, the current working directory will be compared
    /// against the point at which it branched off of origin/master.
    #[clap(name = "changed-since")]
    ChangedSince(ChangedSinceArgs),

    /// Run `cargo clippy`
    #[clap(name = "clippy")]
    Clippy(ClippyArgs),

    /// Run `cargo fix`
    #[clap(name = "fix")]
    Fix(FixArgs),

    /// Run `cargo fmt`
    #[clap(name = "fmt")]
    Fmt(FmtArgs),

    /// Run tests
    #[clap(name = "test")]
    Test(TestArgs),

    /// Run tests with new test runner
    #[clap(name = "nextest")]
    Nextest(NextestArgs),

    /// Run tools
    #[clap(name = "tools")]
    Tools(ToolsArgs),

    /// Run lints
    #[clap(name = "lint")]
    Lint(LintArgs),

    /// Run playground code
    Playground(PlaygroundArgs),

    /// Generate build summaries for important subsets
    #[clap(name = "generate-summaries")]
    GenerateSummaries(GenerateSummariesArgs),

    /// Diff build summaries for important subsets
    #[clap(name = "diff-summary")]
    DiffSummary(DiffSummaryArgs),

    /// Update workspace-hack contents
    #[clap(name = "generate-workspace-hack")]
    GenerateWorkspaceHack(GenerateWorkspaceHackArgs),
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            let color = match record.level() {
                Level::Warn => Color::Yellow,
                Level::Error => Color::Red,
                _ => Color::Green,
            };

            let mut level_style = buf.style();
            level_style.set_color(color).set_bold(true);

            writeln!(
                buf,
                "{:>12} [{}] - {}",
                level_style.value(record.level()),
                Local::now().format("%T%.3f"),
                record.args()
            )
        })
        .init();

    let cli = Cli::parse();
    let xctx = context::XContext::new()?;

    match cli.cmd {
        Command::Tools(args) => tools::run(args, xctx),
        Command::Test(args) => test::run(args, xctx),
        Command::Nextest(args) => nextest::run(args, xctx),
        Command::Build(args) => build::run(args, xctx),
        Command::ChangedSince(args) => changed_since::run(args, xctx),
        Command::Check(args) => check::run(args, xctx),
        Command::Clippy(args) => clippy::run(args, xctx),
        Command::Fix(args) => fix::run(args, xctx),
        Command::Fmt(args) => fmt::run(args, xctx),
        Command::Bench(args) => bench::run(args, xctx),
        Command::Lint(args) => lint::run(args, xctx),
        Command::Playground(args) => playground::run(args, xctx),
        Command::GenerateSummaries(args) => generate_summaries::run(args, xctx),
        Command::DiffSummary(args) => diff_summary::run(args, xctx),
        Command::GenerateWorkspaceHack(args) => generate_workspace_hack::run(args, xctx),
    }
}
