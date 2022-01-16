// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use std::ffi::OsString;

use clap::{ArgEnum, Args};
use supports_color::Stream;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ArgEnum)]
pub enum Coloring {
    Auto,
    Always,
    Never,
}

impl Coloring {
    /// Returns true if the given stream should be colorized.
    pub fn should_colorize(self, stream: Stream) -> bool {
        match self {
            Coloring::Auto => supports_color::on_cached(stream).is_some(),
            Coloring::Always => true,
            Coloring::Never => false,
        }
    }
}

/// Arguments for controlling cargo build and other similar commands (like check).
#[derive(Debug, Args)]
pub struct BuildArgsInner {
    /// No output printed to stdout
    #[clap(long, short)]
    pub(crate) quiet: bool,
    /// Number of parallel build jobs, defaults to # of CPUs
    #[clap(long, short)]
    pub(crate) jobs: Option<u16>,
    /// Only this package's library
    #[clap(long)]
    pub(crate) lib: bool,
    /// Only the specified binary
    #[clap(long, number_of_values = 1)]
    pub(crate) bin: Vec<String>,
    /// All binaries
    #[clap(long)]
    pub(crate) bins: bool,
    /// Only the specified example
    #[clap(long, number_of_values = 1)]
    pub(crate) example: Vec<String>,
    /// All examples
    #[clap(long)]
    pub(crate) examples: bool,
    /// Only the specified test target
    #[clap(long, number_of_values = 1)]
    pub(crate) test: Vec<String>,
    /// All tests
    #[clap(long)]
    pub(crate) tests: bool,
    /// Only the specified bench target
    #[clap(long, number_of_values = 1)]
    pub(crate) bench: Vec<String>,
    /// All benches
    #[clap(long)]
    pub(crate) benches: bool,
    /// All targets
    #[clap(long)]
    pub(crate) all_targets: bool,
    /// Artifacts in release mode, with optimizations
    #[clap(long)]
    pub(crate) release: bool,
    /// Artifacts with the specified profile
    #[clap(long)]
    pub(crate) profile: Option<String>,
    /// Space-separated list of features to activate
    #[clap(long, number_of_values = 1)]
    pub(crate) features: Vec<String>,
    /// Activate all available features
    #[clap(long)]
    pub(crate) all_features: bool,
    /// Do not activate the `default` feature
    #[clap(long)]
    pub(crate) no_default_features: bool,
    /// TRIPLE
    #[clap(long)]
    pub(crate) target: Option<String>,
    /// Directory for all generated artifacts
    #[clap(long, parse(from_os_str))]
    pub(crate) target_dir: Option<OsString>,
    /// Path to Cargo.toml
    #[clap(long, parse(from_os_str))]
    pub(crate) manifest_path: Option<OsString>,
    /// Error format
    #[clap(long)]
    pub(crate) message_format: Option<String>,
    /// Use verbose output (-vv very verbose/build.rs output)
    #[clap(long, short, parse(from_occurrences))]
    pub(crate) verbose: usize,
    /// Coloring: auto, always, never
    #[clap(long, arg_enum, default_value_t = Coloring::Auto)]
    pub(crate) color: Coloring,
    /// Require Cargo.lock and cache are up to date
    #[clap(long)]
    pub(crate) frozen: bool,
    /// Require Cargo.lock is up to date
    #[clap(long)]
    pub(crate) locked: bool,
    /// Run without accessing the network
    #[clap(long)]
    pub(crate) offline: bool,
}

impl BuildArgsInner {
    pub fn add_args(&self, direct_args: &mut Vec<OsString>) {
        if self.quiet {
            direct_args.push(OsString::from("--quiet"));
        }
        if let Some(jobs) = self.jobs {
            direct_args.push(OsString::from("--jobs"));
            direct_args.push(OsString::from(jobs.to_string()));
        };
        if self.lib {
            direct_args.push(OsString::from("--lib"));
        };
        if !self.bin.is_empty() {
            direct_args.push(OsString::from("--bin"));
            for bin in &self.bin {
                direct_args.push(OsString::from(bin));
            }
        }
        if self.bins {
            direct_args.push(OsString::from("--bins"));
        };
        if !self.example.is_empty() {
            direct_args.push(OsString::from("--example"));
            for example in &self.example {
                direct_args.push(OsString::from(example));
            }
        }
        if self.examples {
            direct_args.push(OsString::from("--examples"));
        };

        if !self.test.is_empty() {
            direct_args.push(OsString::from("--test"));
            for test in &self.test {
                direct_args.push(OsString::from(test));
            }
        }
        if self.tests {
            direct_args.push(OsString::from("--tests"));
        };

        if !self.bench.is_empty() {
            direct_args.push(OsString::from("--bench"));
            for bench in &self.bench {
                direct_args.push(OsString::from(bench));
            }
        }
        if self.benches {
            direct_args.push(OsString::from("--benches"));
        };

        if self.all_targets {
            direct_args.push(OsString::from("--all-targets"));
        };
        if self.release {
            direct_args.push(OsString::from("--release"));
        };

        if let Some(profile) = &self.profile {
            direct_args.push(OsString::from("--profile"));
            direct_args.push(OsString::from(profile.to_string()));
        };

        if !self.features.is_empty() {
            direct_args.push(OsString::from("--features"));
            for features in &self.features {
                direct_args.push(OsString::from(features));
            }
        }
        if self.all_features {
            direct_args.push(OsString::from("--all-features"));
        };
        if self.no_default_features {
            direct_args.push(OsString::from("--no-default-features"));
        };

        if let Some(target) = &self.target {
            direct_args.push(OsString::from("--target"));
            direct_args.push(OsString::from(target.to_string()));
        };
        if let Some(target_dir) = &self.target_dir {
            direct_args.push(OsString::from("--target-dir"));
            direct_args.push(OsString::from(target_dir));
        };
        if let Some(manifest_path) = &self.manifest_path {
            direct_args.push(OsString::from("--manifest-path"));
            direct_args.push(manifest_path.to_owned());
        };
        if let Some(message_format) = &self.message_format {
            direct_args.push(OsString::from("--message-format"));
            direct_args.push(OsString::from(message_format.to_string()));
        };
        if self.verbose > 0 {
            direct_args.push(OsString::from(format!("-{}", "v".repeat(self.verbose))));
        };
        if self.color != Coloring::Auto {
            let color = match self.color {
                Coloring::Always => "always",
                Coloring::Never => "never",
                _ => unreachable!(),
            };
            direct_args.push(OsString::from("--color"));
            direct_args.push(OsString::from(color));
        };
        if self.frozen {
            direct_args.push(OsString::from("--frozen"));
        };
        if self.locked {
            direct_args.push(OsString::from("--locked"));
        };
        if self.offline {
            direct_args.push(OsString::from("--offline"));
        };
    }
}
