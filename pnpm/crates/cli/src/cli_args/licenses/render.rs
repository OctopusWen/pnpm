use super::{BelongsTo, LicenseInfo, dependencies::compare_package_names};
use crate::cli_args::sanitize::{sanitize, sanitize_inline};
use indexmap::IndexMap;
use owo_colors::{OwoColorize, Stream};
use std::collections::BTreeMap;
use tabled::{builder::Builder, settings::Style};

pub fn print_license_table(
    results_by_license: &IndexMap<String, BTreeMap<String, LicenseInfo>>,
    long: bool,
) {
    let mut header: Vec<String> = vec!["Package".to_string(), "License".to_string()];
    if long {
        header.push("Details".to_string());
    }

    let mut builder = Builder::default();
    builder.push_record(header);
    for info in sorted_license_infos(results_by_license) {
        let mut row = vec![render_package_name(info), sanitize_inline(&info.license).into_owned()];
        if long {
            row.push(render_license_details(info));
        }
        builder.push_record(row);
    }

    let mut table = builder.build();
    table.with(Style::modern());
    println!("{table}");
}

pub fn render_licenses_json(
    results_by_license: &IndexMap<String, BTreeMap<String, LicenseInfo>>,
) -> miette::Result<String> {
    let mut json_output: IndexMap<String, Vec<&LicenseInfo>> = IndexMap::new();
    for (license, group) in results_by_license {
        let mut infos: Vec<&LicenseInfo> = group.values().collect();
        infos.sort_by(|left, right| compare_package_names(&left.name, &right.name));
        json_output.insert(license.clone(), infos);
    }
    serde_json::to_string_pretty(&json_output)
        .map_err(|error| miette::miette!("Failed to serialize json: {error}"))
}

pub fn render_license_details(info: &LicenseInfo) -> String {
    let details = [info.author.as_ref(), info.description.as_ref(), info.homepage.as_ref()]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    sanitize(&details.join("\n")).into_owned()
}

pub fn render_package_name(info: &LicenseInfo) -> String {
    let name = sanitize_inline(&info.name);
    let suffix = match info.belongs_to {
        BelongsTo::Prod | BelongsTo::Optional => return name.into_owned(),
        BelongsTo::Dev => "(dev)",
    };
    format!("{} {}", name, suffix.if_supports_color(Stream::Stdout, |styled| styled.dimmed()))
}

pub fn sorted_license_infos(
    results_by_license: &IndexMap<String, BTreeMap<String, LicenseInfo>>,
) -> Vec<&LicenseInfo> {
    let mut all_packages: Vec<&LicenseInfo> = results_by_license
        .values()
        .flat_map(BTreeMap::values)
        .collect();
    all_packages.sort_by(|left, right| compare_package_names(&left.name, &right.name));
    all_packages
}
