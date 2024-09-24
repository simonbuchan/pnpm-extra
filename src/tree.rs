use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// The result type for `tree` functionality.
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
#[non_exhaustive]
/// The error type for `tree` functionality.
pub enum Error {
    #[error("could not determine current directory: {0}")]
    /// Error when the current directory cannot be determined.
    CurrentDir(#[source] std::io::Error),

    #[error("could not read pnpm-lock.yaml: {0}")]
    /// Error when the pnpm-lock.yaml file cannot be read.
    ReadLockfile(#[source] std::io::Error),

    #[error("could not parse lockfile structure: {0}")]
    /// Error when the pnpm-lock.yaml file cannot be parsed.
    ParseLockfile(#[source] serde_yaml::Error),

    #[error("Unexpected lockfile content")]
    /// Error when the lockfile content could not be understood.
    /// Currently, this is only when the snapshot key cannot be split into a package name and
    /// version.
    UnexpectedLockfileContent,
}

#[derive(Debug, serde::Deserialize)]
#[non_exhaustive]
#[serde(tag = "lockfileVersion")]
/// A subset of the pnpm-lock.yaml file format.
pub enum Lockfile {
    #[serde(rename = "9.0")]
    /// Only supports version 9.0 currently, though apparently versions are backwards compatible?
    /// https://github.com/orgs/pnpm/discussions/6857
    V9 {
        /// Importers describe the packages in the workspace and their resolved dependencies.
        ///
        /// The key is a relative path to the directory containing the package.json, e.g.:
        /// "packages/foo", or "." for the workspace root.
        importers: HashMap<String, Importer>,

        /// Snapshots describe the packages in the store (e.g. from the registry) and their
        /// resolved dependencies.
        ///
        /// The key is the package name and qualified version, e.g.: "foo@1.2.3",
        /// "bar@4.5.6(peer@7.8.9)", and so on (pnpm code refers to this as the "depPath").
        ///
        /// Note that this key also currently serves as the directory entry in the virtual store,
        /// e.g. "node_modules/.pnpm/{key}", see: https://pnpm.io/how-peers-are-resolved
        snapshots: HashMap<String, Snapshot>,
    },
}

impl Lockfile {
    /// Read the content of a pnpm-lock.yaml file.
    ///
    /// # Errors
    /// - [`Error::ReadLockfile`], if the `pnpm-lock.yaml` file cannot be read from the provided
    ///   workspace directory.
    /// - [`Error::ParseLockfile`], if the data cannot be parsed as a `Lockfile`.
    pub fn read_from_workspace_dir(workspace_dir: &std::path::Path) -> Result<Self> {
        let data =
            std::fs::read(workspace_dir.join("pnpm-lock.yaml")).map_err(Error::ReadLockfile)?;
        Self::from_slice(&data)
    }

    /// Parse the content of a pnpm-lock.yaml file.
    ///
    /// # Errors
    /// - [`Error::ParseLockfile`], if the data cannot be parsed as a `Lockfile`.
    pub fn from_slice(data: &[u8]) -> Result<Self> {
        let result: Self = serde_yaml::from_slice(data).map_err(Error::ParseLockfile)?;
        Ok(result)
    }
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
    /// The specifier from the package.json, e.g. "^1.2.3", "workspace:^", etc.
    pub specifier: String,

    /// The resolved version of the dependency.
    ///
    /// This will be either a qualified version that together with the package name forms a key
    /// into the snapshots map, or a "link:" for workspace packages, e.g.:
    ///
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
#[serde(rename_all = "camelCase")]
/// A snapshot represents a package in the store.
pub struct Snapshot {
    #[serde(default)]
    /// If the package is only used in optional dependencies.
    pub optional: bool,

    #[serde(default)]
    /// The resolved dependencies of the package, a map from package name to qualified version.
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

    #[serde(default)]
    /// As with `dependencies`, but for optional dependencies (including optional peer
    /// dependencies).
    pub optional_dependencies: HashMap<String, String>,

    #[serde(default)]
    /// The package names of peer dependencies of the transitive package dependencies,
    /// excluding direct peer dependencies.
    pub transitive_peer_dependencies: Vec<String>,
}

/// Performs the `pnpm tree {name}` CLI command, printing a user-friendly inverse dependency tree
/// to stdout of the specified package name for the pnpm workspace in the current directory.
///
/// The output format is not specified and may change without a breaking change.
///
/// # Errors
/// - [`Error::ReadLockfile`] If the pnpm-lock.yaml file cannot be read.
/// - [`Error::ParseLockfile`] If the pnpm-lock.yaml file cannot be parsed.
/// - [`Error::UnexpectedLockfileContent`] If the lockfile content could not otherwise be
///   understood.
pub fn print_tree(workspace_dir: &Path, name: &str) -> Result<()> {
    let lockfile = Lockfile::read_from_workspace_dir(workspace_dir)?;

    let graph = DependencyGraph::from_lockfile(&lockfile, workspace_dir)?;

    // Print the tree, skipping repeated nodes.
    let mut seen = HashSet::<NodeId>::new();

    fn print_tree_inner(
        inverse_deps: &DependencyGraph,
        seen: &mut HashSet<NodeId>,
        node_id: &NodeId,
        depth: usize,
    ) {
        if !seen.insert(node_id.clone()) {
            println!("{:indent$}{node_id} (*)", "", indent = depth * 2,);
            return;
        }
        let Some(dep_ids) = inverse_deps.inverse.get(node_id) else {
            println!("{:indent$}{node_id}", "", indent = depth * 2,);
            return;
        };
        println!("{:indent$}{node_id}:", "", indent = depth * 2,);
        for dep_id in dep_ids {
            print_tree_inner(inverse_deps, seen, dep_id, depth + 1);
        }
    }

    for node_id in graph.inverse.keys() {
        if matches!(node_id, NodeId::Package { name: package_name, .. } if name == package_name) {
            print_tree_inner(&graph, &mut seen, node_id, 0);
        }
    }

    Ok(())
}

#[derive(Default)]
/// A dependency graph.
pub struct DependencyGraph {
    /// A map from a node to a set of nodes it depends on.
    pub forward: HashMap<NodeId, HashSet<NodeId>>,

    /// A map from a node to a set of nodes that depend on it.
    pub inverse: HashMap<NodeId, HashSet<NodeId>>,
}

impl DependencyGraph {
    /// Construct a [`DependencyGraph`] from a [`Lockfile`].
    ///
    /// Computes a forwards and inverse dependency graph from the lockfile, used to print
    /// and filter the dependency tree.
    ///
    /// # Errors
    /// - [`Error::UnexpectedLockfileContent`] If the lockfile content could not be understood.
    pub fn from_lockfile(lockfile: &Lockfile, workspace_dir: &Path) -> Result<Self> {
        let Lockfile::V9 {
            importers,
            snapshots,
        } = lockfile;

        let mut forward = HashMap::<NodeId, HashSet<NodeId>>::new();
        let mut inverse = HashMap::<NodeId, HashSet<NodeId>>::new();

        for (path, entry) in importers {
            let path = workspace_dir.join(path);
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
                forward
                    .entry(node_id.clone())
                    .or_default()
                    .insert(dep_id.clone());
                inverse.entry(dep_id).or_default().insert(node_id.clone());
            }
        }

        for (id, entry) in snapshots {
            let split = 1 + id[1..].find('@').ok_or(Error::UnexpectedLockfileContent)?;
            let node_id = NodeId::Package {
                name: id[..split].to_string(),
                version: id[split + 1..].to_string(),
            };
            for (dep_name, dep_version) in &entry.dependencies {
                let dep_id = NodeId::Package {
                    name: dep_name.clone(),
                    version: dep_version.clone(),
                };
                forward
                    .entry(node_id.clone())
                    .or_default()
                    .insert(dep_id.clone());
                inverse.entry(dep_id).or_default().insert(node_id.clone());
            }
        }

        Ok(Self { forward, inverse })
    }
}

#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
/// A node in the dependency graph.
pub enum NodeId {
    /// A package in the workspace.
    Importer {
        /// The workspace-relative path to the package directory.
        path: PathBuf,
    },

    /// A package from the registry.
    Package {
        /// The package name.
        name: String,

        /// The peer-dependency qualified version.
        version: String,
    },
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeId::Importer { path } => write!(f, "{}", path.display()),
            NodeId::Package { name, version } => write!(f, "{}@{}", name, version),
        }
    }
}
