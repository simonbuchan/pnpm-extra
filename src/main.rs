use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
enum Args {
    /// better pnpm why, modelled on cargo tree -i
    Tree {
        #[clap(name = "name")]
        name: String,
    },

    #[clap(subcommand)]
    Catalog(catalog::Args),
}

mod catalog;

fn main() -> Result<()> {
    match Args::parse() {
        Args::Tree { name } => {
            pnpm_extra::tree::print_tree(&name)?;
            Ok(())
        }
        Args::Catalog(args) => {
            catalog::run(args)?;
            Ok(())
        }
    }
}
