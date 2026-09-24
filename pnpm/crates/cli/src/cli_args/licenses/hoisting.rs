use pnpm_config::{Config, NodeLinker};
use pnpm_deps_restorer::VirtualStoreLayout;
use pnpm_lockfile::PackageKey;
use pnpm_modules_yaml::{
    Host as ModulesHost, Modules, NodeLinker as ModulesNodeLinker, read_modules_manifest,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub struct HoistingContext<'a> {
    pub lockfile_dir: &'a Path,
    pub project_dir: &'a Path,
    pub modules_dir_name: &'a Path,
    pub is_hoisted: bool,
    pub is_shamefully_hoist: bool,
    pub hoisted_locations: Option<&'a BTreeMap<String, Vec<String>>>,
}

impl<'a> HoistingContext<'a> {
    pub fn new(
        config: &'a Config,
        lockfile_dir: &'a Path,
        project_dir: &'a Path,
        manifest: Option<&'a Modules>,
    ) -> Self {
        let is_hoisted = config.node_linker == NodeLinker::Hoisted
            || manifest.and_then(|item| item.node_linker) == Some(ModulesNodeLinker::Hoisted);
        let is_shamefully_hoist = config.shamefully_hoist
            || manifest.and_then(|item| item.shamefully_hoist).unwrap_or(false)
            || is_public_hoist_all(manifest);
        let hoisted_locations = manifest.and_then(|item| item.hoisted_locations.as_ref());

        Self {
            lockfile_dir,
            project_dir,
            modules_dir_name: Path::new(&config.modules_dir),
            is_hoisted,
            is_shamefully_hoist,
            hoisted_locations,
        }
    }
}

fn is_public_hoist_all(manifest: Option<&Modules>) -> bool {
    let Some(patterns) = manifest.and_then(|item| item.public_hoist_pattern.as_ref()) else {
        return false;
    };
    patterns
        .iter()
        .any(|pattern| pattern == "*")
}

pub fn read_manifest(modules_dir: &Path) -> Option<Modules> {
    read_modules_manifest::<ModulesHost>(modules_dir).ok().flatten()
}

pub fn resolve_package_dir(
    key: &PackageKey,
    name: &str,
    layout: &VirtualStoreLayout,
    hoisting: &HoistingContext<'_>,
) -> PathBuf {
    if hoisting.is_hoisted {
        resolve_hoisted_path(key, name, hoisting)
    } else if hoisting.is_shamefully_hoist {
        resolve_shameful_path(key, name, layout, hoisting)
    } else {
        virtual_store_package_path(layout, key, name)
    }
}

fn virtual_store_package_path(
    layout: &VirtualStoreLayout,
    key: &PackageKey,
    name: &str,
) -> PathBuf {
    layout
        .slot_dir(key)
        .join("node_modules")
        .join(name)
}

fn resolve_hoisted_path(key: &PackageKey, name: &str, hoisting: &HoistingContext<'_>) -> PathBuf {
    if let Some(path) = find_hoisted_location(key, hoisting) {
        return path;
    }
    candidate_node_modules_path(name, hoisting)
}

fn find_hoisted_location(key: &PackageKey, hoisting: &HoistingContext<'_>) -> Option<PathBuf> {
    let locations = lookup_hoisted_locations(key, hoisting.hoisted_locations)?;
    let paths: Vec<PathBuf> = locations
        .iter()
        .map(|location| hoisting.lockfile_dir.join(location))
        .collect();

    let project_match = paths
        .iter()
        .find(|path| path.starts_with(hoisting.project_dir) && path.exists());
    if let Some(matched) = project_match {
        return Some(matched.clone());
    }

    let existing = paths.iter().find(|path| path.exists());
    if let Some(matched) = existing {
        return Some(matched.clone());
    }

    paths.into_iter().next()
}

fn lookup_hoisted_locations<'a>(
    key: &PackageKey,
    hoisted_map: Option<&'a BTreeMap<String, Vec<String>>>,
) -> Option<&'a Vec<String>> {
    let map = hoisted_map?;
    let key_str = key.to_string();
    if let Some(locations) = map.get(&key_str) {
        return Some(locations);
    }
    let key_no_peer_str = key.without_peer().to_string();
    map.get(&key_no_peer_str)
}

fn candidate_node_modules_path(name: &str, hoisting: &HoistingContext<'_>) -> PathBuf {
    let candidate_project = hoisting.project_dir.join(hoisting.modules_dir_name).join(name);
    if candidate_project.exists() {
        candidate_project
    } else {
        hoisting.lockfile_dir.join(hoisting.modules_dir_name).join(name)
    }
}

fn resolve_shameful_path(
    key: &PackageKey,
    name: &str,
    layout: &VirtualStoreLayout,
    hoisting: &HoistingContext<'_>,
) -> PathBuf {
    let candidate_project = hoisting.project_dir.join(hoisting.modules_dir_name).join(name);
    if candidate_project.exists() {
        return candidate_project;
    }
    let candidate_root = hoisting.lockfile_dir.join(hoisting.modules_dir_name).join(name);
    if candidate_root.exists() {
        return candidate_root;
    }
    virtual_store_package_path(layout, key, name)
}
