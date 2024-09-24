#![doc = include_str!("../README.md")]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
enum Args {
    /// better pnpm why, modelled on cargo tree -i
    Tree {
        #[clap(name = "name")]
        /// Package name to show the tree for
        name: String,

        #[clap(short, long, default_value = ".")]
        /// Workspace directory
        dir: PathBuf,
    },

    #[clap(subcommand)]
    Catalog(catalog::Args),
}

mod catalog;

fn main() -> Result<()> {
    match Args::parse() {
        Args::Tree { name, dir } => {
            let dir = std::path::absolute(dir)?;
            pnpm_extra::tree::print_tree(&dir, &name)?;
            Ok(())
        }
        Args::Catalog(args) => {
            catalog::run(args)?;
            Ok(())
        }
    }
}
