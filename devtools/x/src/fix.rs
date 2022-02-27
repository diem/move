// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use clap::Args;

use crate::{
    cargo::{build_args::BuildArgsInner, selected_package::SelectedPackageArgs, CargoCommand},
    context::XContext,
    Result,
};
use std::ffi::OsString;

#[derive(Debug, Args)]
pub struct FixArgs {
    #[clap(flatten)]
    pub(crate) package_args: SelectedPackageArgs,
    #[clap(flatten)]
    pub(crate) build_args: BuildArgsInner,
    #[clap(name = "ARGS", parse(from_os_str), last = true)]
    args: Vec<OsString>,
}

pub fn run(mut args: FixArgs, xctx: XContext) -> Result<()> {
    let mut pass_through_args = vec![];
    pass_through_args.extend(args.args);

    // Always run fix on all targets.
    args.build_args.all_targets = true;

    let mut direct_args = vec![];
    args.build_args.add_args(&mut direct_args);

    let cmd = CargoCommand::Fix {
        cargo_config: xctx.config().cargo_config(),
        direct_args: &direct_args,
        args: &pass_through_args,
    };
    let packages = args.package_args.to_selected_packages(&xctx)?;
    cmd.run_on_packages(&packages)
}
