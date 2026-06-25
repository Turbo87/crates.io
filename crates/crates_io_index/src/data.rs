use crate::features::FeaturesMap;
use crate::ser::serialize_pubtime;
use chrono::{DateTime, Utc};
use std::cmp::Ordering;

#[derive(Serialize, Deserialize, Debug)]
pub struct Crate {
    pub name: String,
    pub vers: String,
    pub deps: Vec<Dependency>,
    pub cksum: String,
    pub features: FeaturesMap,
    /// This field contains features with new, extended syntax. Specifically,
    /// namespaced features (`dep:`) and weak dependencies (`pkg?/feat`).
    ///
    /// It is only populated if a feature uses the new syntax. Cargo merges it
    /// on top of the `features` field when reading the entries.
    ///
    /// This is separated from `features` because versions older than 1.19
    /// will fail to load due to not being able to parse the new syntax, even
    /// with a `Cargo.lock` file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features2: Option<FeaturesMap>,
    pub yanked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rust_version: Option<String>,
    /// Publication timestamp in ISO8601 format
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_pubtime"
    )]
    pub pubtime: Option<DateTime<Utc>>,
    /// The schema version for this entry.
    ///
    /// If this is None, it defaults to version 1. Entries with unknown
    /// versions are ignored by cargo starting with 1.51.
    ///
    /// Version `2` format adds the `features2` field.
    ///
    /// This provides a method to safely introduce changes to index entries
    /// and allow older versions of cargo to ignore newer entries it doesn't
    /// understand. This is honored as of 1.51, so unfortunately older
    /// versions will ignore it, and potentially misinterpret version 2 and
    /// newer entries.
    ///
    /// The intent is that versions older than 1.51 will work with a
    /// pre-existing `Cargo.lock`, but they may not correctly process `cargo
    /// update` or build a lock from scratch. In that case, cargo may
    /// incorrectly select a new package that uses a new index format. A
    /// workaround is to downgrade any packages that are incompatible with the
    /// `--precise` flag of `cargo update`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub v: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub req: String,
    pub features: Vec<String>,
    pub optional: bool,
    pub default_features: bool,
    pub target: Option<String>,
    pub kind: Option<DependencyKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
}

impl PartialOrd for Dependency {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dependency {
    fn cmp(&self, other: &Self) -> Ordering {
        // In old `cargo` versions the dependency order appears to matter if the
        // same dependency exists twice but with different `kind` fields. In
        // those cases the `optional` field can sometimes be ignored or
        // misinterpreted. With this manual `Ord` implementation we ensure that
        // `normal` dependencies are always first when multiple with the same
        // `name` exist.
        (
            &self.name,
            self.kind,
            &self.req,
            self.optional,
            self.default_features,
            &self.target,
            &self.package,
            &self.features,
        )
            .cmp(&(
                &other.name,
                other.kind,
                &other.req,
                other.optional,
                other.default_features,
                &other.target,
                &other.package,
                &other.features,
            ))
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug, PartialEq, PartialOrd, Ord, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    Normal,
    Build,
    Dev,
}

#[cfg(test)]
mod proptests {
    use super::{Dependency, DependencyKind};
    use hegel::TestCase;
    use hegel::generators as gs;
    use std::cmp::Ordering;

    /// Draws a `Dependency` from small per-field pools so that distinct draws
    /// frequently tie on some fields — which is what actually exercises the
    /// tie-breaking ordering and the total-order axioms.
    fn dependency() -> impl hegel::Generator<Dependency> {
        hegel::compose!(|tc| {
            let strings = |pool: Vec<&str>| {
                pool.into_iter().map(String::from).collect::<Vec<_>>()
            };
            Dependency {
                name: tc.draw(gs::sampled_from(strings(vec!["a", "b", "c"]))),
                req: tc.draw(gs::sampled_from(strings(vec!["^1.0", "=2.0", "*"]))),
                features: tc.draw(
                    gs::vecs(gs::sampled_from(strings(vec!["x", "y"]))).max_size(2),
                ),
                optional: tc.draw(gs::booleans()),
                default_features: tc.draw(gs::booleans()),
                target: tc.draw(gs::sampled_from(vec![
                    None,
                    Some("cfg(unix)".to_string()),
                    Some("wasm".to_string()),
                ])),
                kind: tc.draw(gs::sampled_from(vec![
                    None,
                    Some(DependencyKind::Normal),
                    Some(DependencyKind::Build),
                    Some(DependencyKind::Dev),
                ])),
                package: tc.draw(gs::sampled_from(vec![
                    None,
                    Some("p".to_string()),
                    Some("q".to_string()),
                ])),
            }
        })
    }

    #[hegel::test(test_cases = 1000)]
    fn prop_reflexive(tc: TestCase) {
        let a = tc.draw(dependency());
        assert_eq!(a.cmp(&a), Ordering::Equal);
        assert_eq!(a.partial_cmp(&a), Some(Ordering::Equal));
        assert_eq!(a, a);
    }

    #[hegel::test(test_cases = 3000)]
    fn prop_antisymmetric_and_consistent_with_eq(tc: TestCase) {
        let a = tc.draw(dependency());
        let b = tc.draw(dependency());

        // partial_cmp agrees with cmp.
        assert_eq!(a.partial_cmp(&b), Some(a.cmp(&b)));
        // Reversing the operands reverses the ordering.
        assert_eq!(b.cmp(&a), a.cmp(&b).reverse());
        // cmp reports Equal exactly when the values are equal.
        assert_eq!(a.cmp(&b) == Ordering::Equal, a == b);
    }

    #[hegel::test(test_cases = 4000)]
    fn prop_transitive(tc: TestCase) {
        let a = tc.draw(dependency());
        let b = tc.draw(dependency());
        let c = tc.draw(dependency());

        let (ab, bc, ac) = (a.cmp(&b), b.cmp(&c), a.cmp(&c));
        if ab != Ordering::Greater && bc != Ordering::Greater {
            assert_ne!(ac, Ordering::Greater, "a<=b, b<=c but a>c");
        }
        if ab != Ordering::Less && bc != Ordering::Less {
            assert_ne!(ac, Ordering::Less, "a>=b, b>=c but a<c");
        }
    }

    #[hegel::test(test_cases = 1000)]
    fn prop_sort_yields_ordered_sequence(tc: TestCase) {
        let mut deps = tc.draw(gs::vecs(dependency()).max_size(12));
        deps.sort_by(|a, b| a.cmp(b));
        for pair in deps.windows(2) {
            assert_ne!(pair[0].cmp(&pair[1]), Ordering::Greater, "sort left a > b adjacent");
        }
    }
}
