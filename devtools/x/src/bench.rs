// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use clap::Args;

use crate::{
    cargo::{selected_package::SelectedPackageArgs, CargoCommand},
    context::XContext,
    Result,
};
use std::ffi::OsString;

#[derive(Debug, Args)]
pub struct BenchArgs {
    #[clap(flatten)]
    package_args: SelectedPackageArgs,
    /// Do not run the benchmarks, but compile them
    #[clap(long)]
    no_run: bool,
    #[clap(name = "BENCHNAME", parse(from_os_str))]
    benchname: Option<OsString>,
    #[clap(name = "ARGS", parse(from_os_str), last = true)]
    args: Vec<OsString>,
}

pub fn run(mut args: BenchArgs, xctx: XContext) -> Result<()> {
    args.args.extend(args.benchname.clone());

    let mut direct_args = Vec::new();
    if args.no_run {
        direct_args.push(OsString::from("--no-run"));
    };

    let cmd = CargoCommand::Bench {
        cargo_config: xctx.config().cargo_config(),
        direct_args: direct_args.as_slice(),
        args: &args.args,
        env: &[],
    };

    let packages = args.package_args.to_selected_packages(&xctx)?;
    cmd.run_on_packages(&packages)
}
