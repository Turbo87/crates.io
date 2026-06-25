use std::collections::BTreeMap;

pub type FeaturesMap = BTreeMap<String, Vec<String>>;

/// Splits the given [`FeaturesMap`] into two [`FeaturesMap`]s based on their
/// values.
///
/// See <https://rust-lang.github.io/rfcs/3143-cargo-weak-namespaced-features.html>.
pub fn split_features(
    features: impl IntoIterator<Item = (String, Vec<String>)>,
) -> (FeaturesMap, FeaturesMap) {
    const ITERATION_LIMIT: usize = 100;

    // First, we partition the features into two groups: those that use the new
    // features syntax (`features2`) and those that don't (`features`).
    let (mut features, mut features2): (FeaturesMap, FeaturesMap) = features
        .into_iter()
        .partition(|(_k, vals)| !vals.iter().map(String::as_ref).any(has_features2_syntax));

    // Then, we recursively move features from `features` to `features2` if they
    // depend on features in `features2`.
    for i in (0..ITERATION_LIMIT).rev() {
        let split = features
            .into_iter()
            .partition(|(_k, vals)| !vals.iter().any(|v| features2.contains_key(v)));

        features = split.0;

        if split.1.is_empty() {
            break;
        }

        features2.extend(split.1);

        if i == 0 {
            warn!("Iteration limit reached while splitting features!");
        }
    }

    (features, features2)
}

fn has_features2_syntax(s: &str) -> bool {
    s.starts_with("dep:") || s.contains("?/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::{assert_compact_debug_snapshot, assert_debug_snapshot};
    use serde_json::json;

    #[test]
    fn test_split_features_no_deps() {
        let mut features = FeaturesMap::new();
        features.insert(
            "feature1".to_string(),
            vec!["val1".to_string(), "val2".to_string()],
        );
        features.insert("feature2".to_string(), vec!["val3".to_string()]);

        let (features, features2) = split_features(features);

        assert_compact_debug_snapshot!(features, @r#"{"feature1": ["val1", "val2"], "feature2": ["val3"]}"#);
        assert_compact_debug_snapshot!(features2, @"{}");
    }

    #[test]
    fn test_split_features_with_deps() {
        let mut features = FeaturesMap::new();
        features.insert(
            "feature1".to_string(),
            vec!["dep:val1".to_string(), "val2".to_string()],
        );
        features.insert(
            "feature2".to_string(),
            vec!["val3".to_string(), "val4?/val5".to_string()],
        );

        let (features, features2) = split_features(features);

        assert_compact_debug_snapshot!(features, @"{}");
        assert_compact_debug_snapshot!(features2, @r#"{"feature1": ["dep:val1", "val2"], "feature2": ["val3", "val4?/val5"]}"#);
    }

    #[test]
    fn test_split_features_mixed() {
        let mut features = FeaturesMap::new();
        features.insert(
            "feature1".to_string(),
            vec!["val1".to_string(), "val2".to_string()],
        );
        features.insert("feature2".to_string(), vec!["dep:val3".to_string()]);
        features.insert(
            "feature3".to_string(),
            vec!["val4".to_string(), "val5?/val6".to_string()],
        );

        let (features, features2) = split_features(features);

        assert_compact_debug_snapshot!(features, @r#"{"feature1": ["val1", "val2"]}"#);
        assert_compact_debug_snapshot!(features2, @r#"{"feature2": ["dep:val3"], "feature3": ["val4", "val5?/val6"]}"#);
    }

    #[test]
    fn test_split_features_nested() {
        let mut features = FeaturesMap::new();
        features.insert("feature1".to_string(), vec!["feature2".to_string()]);
        features.insert("feature2".to_string(), vec![]);
        features.insert("feature3".to_string(), vec!["feature1".to_string()]);

        let (features, features2) = split_features(features);

        assert_compact_debug_snapshot!(features, @r#"{"feature1": ["feature2"], "feature2": [], "feature3": ["feature1"]}"#);
        assert_compact_debug_snapshot!(features2, @"{}");
    }

    #[test]
    fn test_split_features_nested_mixed() {
        let mut features = FeaturesMap::new();
        features.insert("feature1".to_string(), vec!["feature2".to_string()]);
        features.insert("feature2".to_string(), vec!["feature3".to_string()]);
        features.insert("feature3".to_string(), vec!["dep:foo".to_string()]);

        let (features, features2) = split_features(features);

        assert_compact_debug_snapshot!(features, @"{}");
        assert_compact_debug_snapshot!(features2, @r#"{"feature1": ["feature2"], "feature2": ["feature3"], "feature3": ["dep:foo"]}"#);
    }

    #[test]
    fn test_split_features_clap() {
        let json = json!({
            "env": ["clap_builder/env"],
            "std": ["clap_builder/std"],
            "help": ["clap_builder/help"],
            "cargo": ["clap_builder/cargo"],
            "color": ["clap_builder/color"],
            "debug": ["clap_builder/debug", "clap_derive?/debug"],
            "usage": ["clap_builder/usage"],
            "derive": ["dep:clap_derive"],
            "string": ["clap_builder/string"],
            "default": ["std", "color", "help", "usage", "error-context", "suggestions"],
            "unicode": ["clap_builder/unicode"],
            "wrap_help": ["clap_builder/wrap_help"],
            "deprecated": ["clap_builder/deprecated", "clap_derive?/deprecated"],
            "suggestions": ["clap_builder/suggestions"],
            "unstable-v5": ["clap_builder/unstable-v5", "clap_derive?/unstable-v5", "deprecated"],
            "unstable-doc": ["clap_builder/unstable-doc", "derive"],
            "unstable-ext": ["clap_builder/unstable-ext"],
            "error-context": ["clap_builder/error-context"],
            "unstable-styles": ["clap_builder/unstable-styles"]
        });

        let features = serde_json::from_value::<FeaturesMap>(json).unwrap();
        assert_debug_snapshot!(split_features(features));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use hegel::generators as gs;
    use hegel::{Generator, TestCase};

    /// Reference for the private `has_features2_syntax`.
    fn has_syntax(s: &str) -> bool {
        s.starts_with("dep:") || s.contains("?/")
    }

    /// Generates a small `FeaturesMap` over a tiny name pool so that values
    /// frequently reference each other's keys (exercising the recursive
    /// propagation), mixed with `dep:` and `?/` syntax tokens. The map is kept
    /// small enough that the 100-iteration safety limit is never reached.
    fn features_map() -> impl Generator<FeaturesMap> {
        const NAMES: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h"];
        hegel::compose!(|tc| {
            let names: Vec<String> = NAMES.iter().map(|s| s.to_string()).collect();
            let mut value_pool = names.clone();
            for n in NAMES {
                value_pool.push(format!("dep:{n}"));
                value_pool.push(format!("{n}?/feat"));
            }
            value_pool.push("plainvalue".to_string());

            let keys = tc.draw(gs::vecs(gs::sampled_from(names)).max_size(8));
            let mut map = FeaturesMap::new();
            for key in keys {
                let vals = tc.draw(gs::vecs(gs::sampled_from(value_pool.clone())).max_size(5));
                map.insert(key, vals);
            }
            map
        })
    }

    #[hegel::test(test_cases = 2000)]
    fn prop_split_features_is_correct_partition(tc: TestCase) {
        let original = tc.draw(features_map());
        let (features, features2) = split_features(original.clone());

        // The two halves are disjoint and together reproduce the input exactly,
        // values included (nothing is lost, duplicated, or mutated).
        for key in features.keys() {
            assert!(!features2.contains_key(key), "key {key:?} in both halves");
        }
        let mut combined = features.clone();
        combined.extend(features2.clone());
        assert_eq!(combined, original, "partition does not reproduce input");

        // `features` entries are clean: no features2 syntax and no reference to a
        // key that lives in `features2` (the closure property).
        for (key, vals) in &features {
            for val in vals {
                assert!(!has_syntax(val), "feature {key:?} keeps syntax value {val:?}");
                assert!(
                    !features2.contains_key(val),
                    "feature {key:?} references features2 key {val:?}"
                );
            }
        }

        // Every `features2` entry is justified: it either uses features2 syntax
        // directly or depends on another features2 key.
        for (key, vals) in &features2 {
            let justified = vals
                .iter()
                .any(|val| has_syntax(val) || features2.contains_key(val));
            assert!(justified, "features2 entry {key:?} is not justified: {vals:?}");
        }
    }

    #[hegel::test(test_cases = 1000)]
    fn prop_split_features_is_idempotent(tc: TestCase) {
        let original = tc.draw(features_map());
        let (features, features2) = split_features(original);

        // Recombining and re-splitting reaches the same fixed point.
        let mut combined = features.clone();
        combined.extend(features2.clone());
        assert_eq!(split_features(combined), (features, features2));
    }
}
