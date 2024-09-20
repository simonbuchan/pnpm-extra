use anyhow::{Context as _, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "lockfileVersion")]
pub enum Lockfile {
    #[serde(rename = "9.0")]
    V9 {
        importers: HashMap<String, Importer>,
        snapshots: HashMap<String, Snapshot>,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Importer {
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Dependency {
    // specifier: String,
    pub version: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

pub fn print_tree(name: &str) -> Result<()> {
    let root = std::env::current_dir().context("getting current directory")?;

    let Lockfile::V9 {
        importers,
        snapshots,
    } = serde_yaml::from_slice::<Lockfile>(
        &std::fs::read(root.join("pnpm-lock.yaml")).context("reading pnpm-lock.yaml")?,
    )
    .context("parsing pnpm-lock.yaml")?;

    // Invert the dependency graph.
    #[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
    enum NodeId {
        Importer { path: PathBuf },
        Package { name: String, version: String },
    }

    impl std::fmt::Display for NodeId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                NodeId::Importer { path } => write!(f, "{}", path.display()),
                NodeId::Package { name, version } => write!(f, "{}@{}", name, version),
            }
        }
    }

    let mut inverse_deps = HashMap::<NodeId, HashSet<NodeId>>::new();

    for (path, entry) in importers {
        let path = root.join(path);
        let node_id = NodeId::Importer { path: path.clone() };
        for (dep_name, dep) in entry
            .dependencies
            .iter()
            .chain(entry.dev_dependencies.iter())
        {
            let dep_id = if let Some(link_path) = dep.version.strip_prefix("link:") {
                NodeId::Importer {
                    path: path.join(link_path),
                }
            } else {
                NodeId::Package {
                    name: dep_name.clone(),
                    version: dep.version.clone(),
                }
            };
            inverse_deps
                .entry(dep_id)
                .or_default()
                .insert(node_id.clone());
        }
    }

    for (id, entry) in snapshots {
        let split = 1 + id[1..].find('@').context("missing @ in id")?;
        let node_id = NodeId::Package {
            name: id[..split].to_string(),
            version: id[split + 1..].to_string(),
        };
        for (dep_name, dep_version) in &entry.dependencies {
            let dep_id = NodeId::Package {
                name: dep_name.clone(),
                version: dep_version.clone(),
            };
            inverse_deps
                .entry(dep_id)
                .or_default()
                .insert(node_id.clone());
        }
    }

    // Print the tree, skipping repeated nodes.
    let mut seen = HashSet::<NodeId>::new();

    fn print_tree_inner(
        inverse_deps: &HashMap<NodeId, HashSet<NodeId>>,
        seen: &mut HashSet<NodeId>,
        node_id: &NodeId,
        depth: usize,
    ) {
        if !seen.insert(node_id.clone()) {
            println!("{:indent$}{node_id} (*)", "", indent = depth * 2,);
            return;
        }
        let Some(dep_ids) = inverse_deps.get(node_id) else {
            println!("{:indent$}{node_id}", "", indent = depth * 2,);
            return;
        };
        println!("{:indent$}{node_id}:", "", indent = depth * 2,);
        for dep_id in dep_ids {
            print_tree_inner(inverse_deps, seen, dep_id, depth + 1);
        }
    }

    for node_id in inverse_deps.keys() {
        if matches!(node_id, NodeId::Package { name: package_name, .. } if name == package_name) {
            print_tree_inner(&inverse_deps, &mut seen, node_id, 0);
        }
    }

    Ok(())
}
