use anyhow::{Context as _, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "lockfileVersion")]
/// A subset of the pnpm-lock.yaml file format.
pub enum Lockfile {
    #[serde(rename = "9.0")]
    /// Only supports version 9.0 currently, though apparently versions are backwards compatible?
    /// https://github.com/orgs/pnpm/discussions/6857
    V9 {
        /// Importers describe the packages in the workspace and their resolved dependencies.
        /// The key is a relative path to the directory containing the package.json, e.g.:
        /// "packages/foo", or "." for the workspace root.
        importers: HashMap<String, Importer>,
        /// Snapshots describe the packages in the store (e.g. from the registry) and their
        /// resolved dependencies.
        ///
        /// The key is the package name and qualified version, e.g.:
        /// "foo@1.2.3", "bar@4.5.6(peer@7.8.9)", and so on.
        ///
        /// Note that this key also currently serves as the directory entry in the store, e.g.
        /// "node_modules/.pnpm/{key}" (which then contains a `node_modules` directory to implement
        /// the dependency resolution).
        snapshots: HashMap<String, Snapshot>,
    },
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// An importer represents a package in the workspace.
pub struct Importer {
    #[serde(default)]
    /// The resolutions of the `dependencies` entry in the package.json.
    /// The key is the package name.
    pub dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    /// The resolutions of the `devDependencies` entry in the package.json.
    /// The key is the package name.
    pub dev_dependencies: HashMap<String, Dependency>,
}

#[derive(Debug, serde::Deserialize)]
/// A dependency represents a resolved dependency for a Importer (workspace package)
pub struct Dependency {
    // specifier: String,
    /// The resolved version of the dependency.
    /// This will be either a qualified version that together with the package name forms a key
    /// into the snapshots map, or a "link:" for workspace packages, e.g.:
    /// ```yaml
    /// ...
    /// importers:
    ///   packages/foo:
    ///     dependencies:
    ///       bar:
    ///         specifier: workspace:^
    ///         version: link:../bar
    ///       baz:
    ///         specifier: ^1.2.0
    ///         version: 1.2.3(peer@4.5.6)
    ///   packages/bar:
    ///     dependencies:
    ///       baz:
    ///         specifier: ^1.2.0
    ///         version: 1.2.3(peer@7.8.9)
    /// ...
    /// ```
    pub version: String,
}

#[derive(Debug, serde::Deserialize)]
/// A snapshot represents a package in the store.
pub struct Snapshot {
    #[serde(default)]
    /// The resolved dependencies of the package, a map from package name to qualified version.
    /// No distinction is made between different dependency kinds here, e.g. `dependencies` vs
    /// `peerDependencies` (and of course, `devDependencies` are not considered).
    /// ```yaml
    /// ...
    /// snapshots:
    ///   foo@1.2.3:
    ///     dependencies:
    ///       bar: 4.5.6
    ///   bar@4.5.6: {}
    /// ...
    /// ```
    pub dependencies: HashMap<String, String>,
}

/// Performs the `pnpm tree {name}` CLI command, printing a user-friendly inverse dependency tree
/// to stdout of the specified package name for the pnpm workspace in the current directory.
///
/// The output format is not specified and may change without a breaking change.
///
/// # Errors
/// - If the current directory cannot be determined.
/// - If the pnpm-lock.yaml file cannot be read or parsed.
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
