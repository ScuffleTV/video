use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;

#[derive(Debug, Clone, serde_derive::Deserialize)]
#[serde(default)]
pub struct XTaskMetadata {
    /// Allows you to provide a list of combinations that should be skipped.
    /// Meaning, that these combinations will not be tested on supersets.
    #[serde(alias = "skip-feature-sets")]
    pub skip_feature_sets: BTreeSet<BTreeSet<String>>,
    /// Allows you to skip optional dependencies.
    #[serde(alias = "skip-optional-dependencies")]
    pub skip_optional_dependencies: bool,
    /// Allows you to provide a list of extra features that should be tested.
    #[serde(alias = "extra-features")]
    pub extra_features: BTreeSet<String>,
    /// Allows you to provide a list of features that should be denied.
    #[serde(alias = "deny-list")]
    pub deny_list: BTreeSet<String>,
    /// Allows you to provide a list of features that should be always included.
    #[serde(alias = "always-include-features")]
    pub always_include_features: BTreeSet<String>,
    /// Allows you to provide the maximum number of features that should be tested.
    #[serde(alias = "max-combination-size")]
    pub max_combination_size: Option<usize>,
    /// Allows you to provide a list of features that should be allowed.
    #[serde(alias = "allow-list")]
    pub allow_list: BTreeSet<String>,
    /// A set of features that are considered strictly additive meaning that they only
    /// add new functionality and do not change the behavior of the crate.
    #[serde(alias = "additive-features")]
    pub additive_features: BTreeSet<String>,
    /// Whether to skip the power set.
    pub skip: bool,
}

impl Default for XTaskMetadata {
    fn default() -> Self {
        Self {
            skip_feature_sets: Default::default(),
            skip_optional_dependencies: true,
            extra_features: Default::default(),
            deny_list: Default::default(),
            always_include_features: Default::default(),
            max_combination_size: None,
            allow_list: Default::default(),
            additive_features: Default::default(),
            skip: false,
        }
    }
}

impl XTaskMetadata {
    pub fn from_package(package: &cargo_metadata::Package) -> anyhow::Result<Self> {
        let Some(metadata) = package.metadata.get("xtask").and_then(|v| v.get("powerset")) else {
            return Ok(Self::default());
        };

        serde_json::from_value(metadata.clone()).context("xtask")
    }
}

fn find_permutations<'a>(
    initial_start: BTreeSet<&'a str>,
    remaining: usize,
    permutations: &mut BTreeSet<BTreeSet<&'a str>>,
    viable_features: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    skip_feature_sets: &BTreeSet<BTreeSet<&'a str>>,
) {
    let mut stack = vec![(initial_start, remaining)];

    while let Some((start, rem)) = stack.pop() {
        if skip_feature_sets.iter().any(|s| s.is_subset(&start)) || !permutations.insert(start.clone()) || rem == 0 {
            continue;
        }

        let flattened: BTreeSet<_> = start
            .iter()
            .flat_map(|f| viable_features[f].iter().chain(std::iter::once(f)))
            .collect();

        for (feature, deps) in viable_features.iter() {
            if flattened.contains(feature) {
                continue;
            }

            let mut new_start = start.clone();
            new_start.insert(feature);
            for dep in deps {
                new_start.remove(dep);
            }

            if permutations.contains(&new_start) || skip_feature_sets.contains(&new_start) {
                continue;
            }

            stack.push((new_start, rem.saturating_sub(1)));
        }
    }
}

fn flatten_features<'a>(deps: &[&'a str], package_features: &BTreeMap<&'a str, Vec<&'a str>>) -> BTreeSet<&'a str> {
    let mut next: Vec<_> = deps.iter().collect();

    let mut features = BTreeSet::new();
    while let Some(dep) = next.pop() {
        if let Some(deps) = package_features.get(dep) {
            if features.insert(*dep) {
                next.extend(deps);
            }
        }
    }

    features
}

pub fn test_package_features<'a>(
    package: &'a cargo_metadata::Package,
    added_features: impl IntoIterator<Item = &'a str>,
    excluded_features: impl IntoIterator<Item = &'a str>,
    xtask_metadata: &'a XTaskMetadata,
) -> anyhow::Result<BTreeSet<BTreeSet<&'a str>>> {
    if package.features.is_empty() {
        return Ok(BTreeSet::new());
    }

    let mut always_included_features: BTreeSet<_> = xtask_metadata
        .always_include_features
        .iter()
        .map(|f| f.as_str())
        .chain(added_features)
        .collect();
    let skip_feature_sets: BTreeSet<_> = excluded_features
        .into_iter()
        .map(|f| BTreeSet::from_iter(std::iter::once(f)))
        .chain(
            xtask_metadata
                .skip_feature_sets
                .iter()
                .map(|s| s.iter().map(|f| f.as_str()).collect()),
        )
        .collect();

    let mut package_features: BTreeMap<&str, Vec<&str>> = package
        .features
        .iter()
        .map(|(k, v)| (k.as_str(), v.iter().map(|f| f.as_str()).collect()))
        .collect();

    if xtask_metadata.skip_optional_dependencies {
        let mut implicit_features = BTreeSet::new();
        let mut used_deps = BTreeSet::new();

        for (feature, deps) in package.features.iter() {
            for dep in deps.iter().filter_map(|f| f.strip_prefix("dep:")) {
                if dep == feature && deps.len() == 1 {
                    implicit_features.insert(feature.as_str());
                } else {
                    used_deps.insert(dep);
                }
            }
        }

        for feature in implicit_features {
            if used_deps.contains(&feature) || xtask_metadata.extra_features.contains(feature) {
                continue;
            }

            package_features.remove(feature);
        }
    }

    let use_allow_list = !xtask_metadata.allow_list.is_empty();
    let use_deny_list = !xtask_metadata.deny_list.is_empty();

    if use_allow_list && use_deny_list {
        anyhow::bail!("Cannot specify both allow and deny lists, please specify only one.");
    }

    let mut viable_features = BTreeMap::new();

    let mut additive_features = BTreeMap::new();

    for (feature, deps) in package_features.iter() {
        // If we are using an allow list, only include features that are in the allow
        // list If we are using a deny list, skip features that are in the deny list
        if (use_allow_list && !xtask_metadata.allow_list.contains(*feature))
            || (use_deny_list && xtask_metadata.deny_list.contains(*feature))
        {
            continue;
        }

        let flattened = flatten_features(deps, &package_features);

        if !xtask_metadata.additive_features.contains(*feature) {
            viable_features.insert(*feature, flattened);
        } else {
            additive_features.insert(*feature, flattened);
        }
    }

    // Remove features that are not in the package
    always_included_features.retain(|f| package_features.contains_key(f));

    // Non additive permutations are permutations that we need to find every
    // combination of
    let mut non_additive_permutations = BTreeSet::new();

    // This finds all the combinations of features that are not additive
    find_permutations(
        always_included_features.clone(),
        xtask_metadata.max_combination_size.unwrap_or(viable_features.len() + 1),
        &mut non_additive_permutations,
        &viable_features,
        &skip_feature_sets,
    );

    // This finds all the combinations of features that are additive
    // With additive features we do not need to find every combination, we just need
    // to add the additive features to the non additive permutations

    // This loop adds the additive features to the non additive permutations
    // Example:
    // - NON_ADDITIVE = [(A), (B), (A, B), ()]
    // - ADDITIVE = [(C), (D), (E)]
    // Result: [
    //   (),
    //   (A),
    //   (B),
    //   (A, B),
    //   (A, C),
    //   (A, C, D),
    //   (A, C, D, E),
    //   (B, C),
    //   (B, C, D),
    //   (B, C, D, E),
    //   (A, B, C),
    //   (A, B, C, D),
    //   (A, B, C, D, E),
    //   (C),
    //   (D),
    //   (E),
    //   (C, D),
    //   (C, D, E),
    // ]
    // To note: we do not test for combinations of the additive features. Such as
    // (A, D, E).

    let mut permutations = BTreeSet::new();
    for mut permutation in non_additive_permutations {
        let flattened: BTreeSet<_> = permutation
            .clone()
            .into_iter()
            .flat_map(|f| viable_features[&f].iter().copied().chain(std::iter::once(f)))
            .collect();

        permutations.insert(permutation.clone());
        for (feature, deps) in additive_features.iter() {
            if flattened.contains(feature) {
                continue;
            }

            permutation.insert(feature);
            for dep in deps {
                permutation.remove(dep);
            }
            permutations.insert(permutation.clone());
        }
    }

    let flattened: BTreeSet<_> = always_included_features
        .iter()
        .flat_map(|f| viable_features[f].iter().chain(std::iter::once(f)))
        .collect();

    for feature in additive_features.keys() {
        if flattened.contains(feature) {
            continue;
        }

        let mut permutation = always_included_features.clone();
        permutation.insert(feature);
        permutations.insert(permutation);
    }

    Ok(permutations)
}

pub fn parse_features<'a>(
    features: impl IntoIterator<Item = &'a str>,
) -> (BTreeSet<&'a str>, BTreeMap<&'a str, BTreeSet<&'a str>>) {
    let mut generic_features = BTreeSet::new();
    let mut crate_features = BTreeMap::new();

    for feature in features {
        let mut splits = feature.splitn(2, '/');
        let first = splits.next().unwrap();
        if let Some(second) = splits.next() {
            crate_features.entry(first).or_insert_with(BTreeSet::new).insert(second);
        } else {
            generic_features.insert(first);
        }
    }

    (generic_features, crate_features)
}
