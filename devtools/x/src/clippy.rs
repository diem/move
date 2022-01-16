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
pub struct ClippyArgs {
    #[clap(flatten)]
    pub(crate) package_args: SelectedPackageArgs,
    #[clap(flatten)]
    pub(crate) build_args: BuildArgsInner,
    #[clap(name = "ARGS", parse(from_os_str), last = true)]
    args: Vec<OsString>,
}

pub fn run(args: ClippyArgs, xctx: XContext) -> Result<()> {
    let mut pass_through_args = vec!["-D".into(), "warnings".into()];
    for lint in xctx.config().allowed_clippy_lints() {
        pass_through_args.push("-A".into());
        pass_through_args.push(lint.into());
    }
    for lint in xctx.config().warn_clippy_lints() {
        pass_through_args.push("-W".into());
        pass_through_args.push(lint.into());
    }
    pass_through_args.extend(args.args);

    let mut direct_args = vec![];
    args.build_args.add_args(&mut direct_args);

    let cmd = CargoCommand::Clippy {
        cargo_config: xctx.config().cargo_config(),
        direct_args: &direct_args,
        args: &pass_through_args,
    };
    let packages = args.package_args.to_selected_packages(&xctx)?;
    cmd.run_on_packages(&packages)
}
