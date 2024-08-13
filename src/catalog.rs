use anyhow::{bail, Context as _, Result};

#[derive(clap::Subcommand)]
pub(crate) enum Args {
    /// pnpm add but for catalog
    Add {
        #[clap(name = "name")]
        /// package name to add
        name: String,

        #[clap(long)]
        /// catalog name to add to, default to "default"
        catalog: Option<String>,
    },
}

fn read_workspace() -> Result<serde_yaml::Mapping> {
    let data = std::fs::read("pnpm-workspace.yaml").context("reading pnpm-workspace.yaml");
    let workspace = serde_yaml::from_slice::<serde_yaml::Value>(&data?)
        .context("parsing pnpm-workspace.yaml")?;
    let workspace = match workspace {
        serde_yaml::Value::Mapping(map) => map,
        _ => bail!("pnpm-workspace.yaml content is not a mapping?"),
    };
    Ok(workspace)
}

pub(crate) fn run(args: Args) -> Result<()> {
    match args {
        Args::Add { name, catalog } => {
            let mut workspace = read_workspace()?;
            let catalog = match catalog {
                None => workspace.entry("catalog".into()),
                Some(catalog) => workspace
                    .entry("catalogs".into())
                    .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()))
                    .as_mapping_mut()
                    .context("catalogs is not a mapping")?
                    .entry(catalog.into()),
            }
            .or_insert_with(|| serde_yaml::Value::Mapping(Default::default()))
            .as_mapping_mut()
            .context("catalog is not a mapping")?;
            if let Some(version) = catalog.get(&name) {
                println!("{} is already in catalog with version {:?}", name, version);
                return Ok(());
            }

            println!("resolving {}", name);
            let output = std::process::Command::new("pnpm")
                .arg("view")
                .arg("--json")
                .arg(&name)
                .output()
                .context("running pnpm view")?;
            let pkg = serde_json::from_slice::<serde_json::Value>(&output.stdout)
                .context("reading pnpm view output json")?;
            let version = pkg["dist-tags"]["latest"]
                .as_str()
                .context("latest version not found")?;
            println!("found {}@{}", name, version);

            catalog.insert(name.into(), format!("^{}", version).into());

            // This will write somewhat ugly yaml, no line separators and single-quoted strings.
            std::fs::write(
                "pnpm-workspace.yaml",
                serde_yaml::to_string(&workspace).context("serializing pnpm-workspace.yaml")?,
            )
            .context("writing pnpm-workspace.yaml")?;
            // So run prettier on it:
            std::process::Command::new("pnpm")
                .arg("exec")
                .arg("--")
                .arg("prettier")
                .arg("--write")
                .arg("pnpm-workspace.yaml")
                .status()
                .context("running prettier")?;

            Ok(())
        }
    }
}
