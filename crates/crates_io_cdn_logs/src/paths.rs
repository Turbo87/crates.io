use semver::Version;
use tracing::instrument;

/// Parse crate name and version from a download URL or URL path.
///
/// This function supports both URL formats:
///
/// - `https://static.crates.io/crates/foo/foo-1.2.3.crate`
/// - `https://static.crates.io/crates/foo/1.2.3/download`
#[instrument(level = "debug")]
pub fn parse_path(mut path: &str) -> Option<(String, Version)> {
    // This would ideally use a regular expression to simplify the code, but
    // regexes are slow, and we want to keep this code as fast as possible.

    // Remove any query parameters.
    if let Some(pos) = path.find('?') {
        path = &path[..pos];
    }

    // Find the start of the path. We assume that we don't have any nested
    // `crates` folders on the server (e.g. `/foo/crates/...`).
    let pos = path.find("/crates/")?;
    let path = &path[pos + 8..];

    // The following code supports both `foo/1.2.3/download`
    // and `foo/foo-1.2.3.crate`
    let (folder, rest) = path.split_once('/')?;
    let version = rest.strip_suffix("/download").or_else(|| {
        rest.strip_suffix(".crate")
            .and_then(|rest| rest.strip_prefix(folder))
            .and_then(|rest| rest.strip_prefix('-'))
    })?;

    let version = Version::parse(version).ok()?;

    Some((folder.to_owned(), version))
}

#[cfg(test)]
mod tests {
    use super::*;
    use claims::{assert_none, assert_some};
    use semver::Version;

    fn format((name, version): &(String, Version)) -> String {
        format!("{name}@{version}")
    }

    #[test]
    fn test_parse_path_valid() {
        let result = assert_some!(parse_path("/crates/foo/foo-1.2.3.crate"));
        assert_eq!(format(&result), "foo@1.2.3");

        let result = assert_some!(parse_path("/crates/foo/1.2.3/download"));
        assert_eq!(format(&result), "foo@1.2.3");
    }

    #[test]
    fn test_parse_path_with_query_params() {
        let result = assert_some!(parse_path("/crates/foo/foo-1.2.3.crate?param=value"));
        assert_eq!(format(&result), "foo@1.2.3");

        let result = assert_some!(parse_path("/crates/foo/1.2.3/download"));
        assert_eq!(format(&result), "foo@1.2.3");
    }

    #[test]
    fn test_parse_path_with_full_url() {
        let path = "https://static.crates.io/crates/foo/foo-1.2.3.crate";
        let result = assert_some!(parse_path(path));
        assert_eq!(format(&result), "foo@1.2.3");

        let path = "https://static.crates.io/crates/foo/1.2.3/download";
        let result = assert_some!(parse_path(path));
        assert_eq!(format(&result), "foo@1.2.3");
    }

    #[test]
    fn test_parse_path_with_dashes() {
        let path = "/crates/foo-bar/foo-bar-1.0.0-rc.1.crate";
        let result = assert_some!(parse_path(path));
        assert_eq!(format(&result), "foo-bar@1.0.0-rc.1");

        let path = "/crates/foo-bar/1.0.0-rc.1/download";
        let result = assert_some!(parse_path(path));
        assert_eq!(format(&result), "foo-bar@1.0.0-rc.1");
    }

    #[test]
    fn test_parse_path_empty() {
        assert_none!(parse_path(""));
    }

    #[test]
    fn test_parse_path_only_query_params() {
        assert_none!(parse_path("?param=value"));
    }

    #[test]
    fn test_parse_path_only_crates_prefix() {
        assert_none!(parse_path("/crates/"));
    }

    #[test]
    fn test_parse_path_unrelated_path() {
        assert_none!(parse_path("/readmes/foo/foo-1.2.3.crate"));
    }

    #[test]
    fn test_parse_path_no_folder() {
        assert_none!(parse_path("/crates/foo-1.2.3.crate"));
    }

    #[test]
    fn test_parse_path_no_file_extension() {
        assert_none!(parse_path("/crates/foo/foo-1.2.3"));
    }

    #[test]
    fn test_parse_path_wrong_file_extension() {
        assert_none!(parse_path("/crates/foo/foo-1.2.3.html"));
    }

    #[test]
    fn test_parse_path_bad_crate_name() {
        assert_none!(parse_path("/crates/foo/bar-1.2.3.crate"));
    }

    #[test]
    fn test_parse_path_invalid_separator() {
        assert_none!(parse_path("/crates/foo/foo@1.2.3.crate"));
    }

    #[test]
    fn test_parse_path_no_version() {
        assert_none!(parse_path("/crates/foo/foo.crate"));
    }

    #[test]
    fn test_parse_path_invalid_version() {
        assert_none!(parse_path("/crates/foo/foo-1.2.3§foo.crate"));
    }
}

#[cfg(test)]
mod proptests {
    use super::parse_path;
    use hegel::TestCase;
    use hegel::generators as gs;
    use semver::Version;

    /// A valid crate name: non-empty, no `/` or `?`. Dashes/dots are allowed and
    /// deliberately exercised because the `.crate` form embeds the name as a
    /// prefix of the filename.
    fn crate_name() -> impl hegel::Generator<String> {
        gs::text()
            .alphabet("abcdefghijklmnopqrstuvwxyz0123456789-_.")
            .min_size(1)
            .max_size(12)
    }

    /// A valid semver version, sometimes with a prerelease and/or build metadata.
    fn version() -> impl hegel::Generator<Version> {
        hegel::compose!(|tc| {
            let major = tc.draw(gs::integers::<u64>().min_value(0).max_value(999));
            let minor = tc.draw(gs::integers::<u64>().min_value(0).max_value(999));
            let patch = tc.draw(gs::integers::<u64>().min_value(0).max_value(999));
            let mut s = format!("{major}.{minor}.{patch}");
            if tc.draw(gs::booleans()) {
                s.push('-');
                s.push_str(&tc.draw(gs::sampled_from(vec![
                    "alpha", "beta", "rc.1", "0.3", "x.7.z.92",
                ])));
            }
            if tc.draw(gs::booleans()) {
                s.push('+');
                s.push_str(&tc.draw(gs::sampled_from(vec!["build", "build.5", "21AF26D3"])));
            }
            Version::parse(&s).unwrap()
        })
    }

    fn host_prefix() -> impl hegel::Generator<String> {
        gs::sampled_from(vec![
            String::new(),
            "https://static.crates.io".to_string(),
            "https://example.com/cdn".to_string(),
        ])
    }

    #[hegel::test(test_cases = 2000)]
    fn prop_roundtrip_both_url_formats(tc: TestCase) {
        let name = tc.draw(crate_name());
        let ver = tc.draw(version());
        let prefix = tc.draw(host_prefix());

        let crate_url = format!("{prefix}/crates/{name}/{name}-{ver}.crate");
        let download_url = format!("{prefix}/crates/{name}/{ver}/download");

        assert_eq!(
            parse_path(&crate_url),
            Some((name.clone(), ver.clone())),
            "crate url = {crate_url:?}"
        );
        assert_eq!(
            parse_path(&download_url),
            Some((name, ver)),
            "download url = {download_url:?}"
        );
    }

    #[hegel::test(test_cases = 1500)]
    fn prop_query_params_are_ignored(tc: TestCase) {
        let name = tc.draw(crate_name());
        let ver = tc.draw(version());
        let url = format!("/crates/{name}/{name}-{ver}.crate");
        let query = tc.draw(gs::text().max_size(20));

        assert_eq!(
            parse_path(&url),
            parse_path(&format!("{url}?{query}")),
            "url = {url:?}, query = {query:?}"
        );
    }

    /// Arbitrary and path-shaped input must never panic.
    #[hegel::test(test_cases = 3000)]
    fn prop_never_panics(tc: TestCase) {
        let tokens: Vec<String> = ["/crates/", "/", "-", ".crate", "/download", "?", "1.2.3", "foo"]
            .into_iter()
            .map(String::from)
            .collect();
        let input = tc.draw(hegel::compose!(|tc| {
            if tc.draw(gs::booleans()) {
                tc.draw(gs::vecs(gs::sampled_from(tokens.clone())).max_size(16)).join("")
            } else {
                tc.draw(gs::text().max_size(40))
            }
        }));
        let _ = parse_path(&input);
    }
}
