//! # pnpm-extra
//!
//! The library for the `pnpm-extra` binary.
//!
//! At the moment this simply exports the `print_tree` function that implments the `pnpm-extra tree`
//! command, and the `read_workspace` function that reads the `pnpm-workspace.yaml` file used by
//! `pnpm-extra catalog add` command.
//!
//! There is no expectation of stability or backwards compatibility for this library at this time,
//! use at your own risk!

use anyhow::{bail, Context as _, Result};

mod tree;

pub use tree::print_tree;

/// Parse and return the content of pnpm-workspace.yaml as a serde_yaml::Mapping.
///
/// This is unlikely to be useful as this is a human edited file and serde_yaml does not preserve
/// comments or formatting, but is provided for completeness.
///
/// # Errors
/// - If the pnpm-workspace.yaml file cannot be read or parsed.
pub fn read_workspace() -> Result<serde_yaml::Mapping> {
    let data = std::fs::read("pnpm-workspace.yaml").context("reading pnpm-workspace.yaml");
    let workspace = serde_yaml::from_slice::<serde_yaml::Value>(&data?)
        .context("parsing pnpm-workspace.yaml")?;
    let workspace = match workspace {
        serde_yaml::Value::Mapping(map) => map,
        _ => bail!("pnpm-workspace.yaml content is not a mapping?"),
    };
    Ok(workspace)
}
