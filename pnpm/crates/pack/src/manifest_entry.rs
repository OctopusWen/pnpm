/// True when a `package/<path>` tar key names the package manifest.
///
/// `pack` rewrites every `package.json` / `package.json5` /
/// `package.yaml` entry into a single serialized `package/package.json`,
/// and reports all of them as `package.json` in the contents listing.
/// Mirrors the `/^package\/package\.(?:json|json5|yaml)$/` test pnpm
/// applies in three places (tar entry rewrite, size, and contents).
#[must_use]
pub fn is_manifest_entry(name: &str) -> bool {
    let Some(basename) = name.strip_prefix("package/") else {
        return false;
    };
    if basename.contains('/') {
        return false;
    }
    let lower = basename.to_ascii_lowercase();
    matches!(lower.as_str(), "package.json" | "package.json5" | "package.yaml")
}
