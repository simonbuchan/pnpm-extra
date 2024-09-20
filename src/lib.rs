use anyhow::{bail, Context as _, Result};

pub mod tree;

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
