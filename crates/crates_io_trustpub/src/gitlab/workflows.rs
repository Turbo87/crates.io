/// Extracts the workflow path from a GitLab `ci_config_ref_uri` claim.
///
/// In other words, it turns e.g. `gitlab.com/rust-lang/regex//foo/bar/baz.yml@refs/heads/main`
/// into `foo/bar/baz.yml`, or `None` if the reference is in an unexpected format.
///
/// This was initially using a regular expression (`//(.*[^/]\.(yml|yaml))@.+`),
/// but was changed to string operations to avoid potential ReDoS attack vectors
/// (see `test_extract_workflow_filename_redos` test below).
pub(crate) fn extract_workflow_filepath(workflow_ref: &str) -> Option<&str> {
    // Find the double slash that separates project path from workflow path
    let start = workflow_ref.find("//")?;
    let after_double_slash = &workflow_ref[start + 2..];

    // Find the last @ that separates workflow path from ref
    let end = after_double_slash.rfind('@')?;
    let filepath = &after_double_slash[..end];

    // Validate: must end with .yml or .yaml
    if !filepath.ends_with(".yml") && !filepath.ends_with(".yaml") {
        return None;
    }

    // Get the basename (part after last slash, or whole string if no slash)
    let basename = filepath.rsplit('/').next()?;

    // Basename must not be empty aside from extension (rejects ".yml", ".yaml", "somedir/.yaml")
    if basename == ".yml" || basename == ".yaml" {
        return None;
    }

    Some(filepath)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_extract_workflow_filename() {
        let test_cases = [
            // Well-formed `ci_config_ref_uri`s, including obnoxious ones.
            (
                "gitlab.com/foo/bar//notnested.yml@/some/ref",
                Some("notnested.yml"),
            ),
            (
                "gitlab.com/foo/bar//notnested.yaml@/some/ref",
                Some("notnested.yaml"),
            ),
            (
                "gitlab.com/foo/bar//basic/basic.yml@/some/ref",
                Some("basic/basic.yml"),
            ),
            (
                "gitlab.com/foo/bar//more/nested/example.yml@/some/ref",
                Some("more/nested/example.yml"),
            ),
            (
                "gitlab.com/foo/bar//too//many//slashes.yml@/some/ref",
                Some("too//many//slashes.yml"),
            ),
            ("gitlab.com/foo/bar//has-@.yml@/some/ref", Some("has-@.yml")),
            (
                "gitlab.com/foo/bar//foo.bar.yml@/some/ref",
                Some("foo.bar.yml"),
            ),
            (
                "gitlab.com/foo/bar//foo.yml.bar.yml@/some/ref",
                Some("foo.yml.bar.yml"),
            ),
            (
                "gitlab.com/foo/bar//foo.yml@bar.yml@/some/ref",
                Some("foo.yml@bar.yml"),
            ),
            (
                "gitlab.com/foo/bar//@foo.yml@bar.yml@/some/ref",
                Some("@foo.yml@bar.yml"),
            ),
            (
                "gitlab.com/foo/bar//@.yml.foo.yml@bar.yml@/some/ref",
                Some("@.yml.foo.yml@bar.yml"),
            ),
            ("gitlab.com/foo/bar//a.yml@refs/heads/main", Some("a.yml")),
            (
                "gitlab.com/foo/bar//a/b.yml@refs/heads/main",
                Some("a/b.yml"),
            ),
            (
                "gitlab.com/foo/bar//.gitlab-ci.yml@refs/heads/main",
                Some(".gitlab-ci.yml"),
            ),
            (
                "gitlab.com/foo/bar//.gitlab-ci.yaml@refs/heads/main",
                Some(".gitlab-ci.yaml"),
            ),
            // Malformed `ci_config_ref_uri`s.
            ("gitlab.com/foo/bar//notnested.wrongsuffix@/some/ref", None),
            ("gitlab.com/foo/bar//@/some/ref", None),
            ("gitlab.com/foo/bar//.yml@/some/ref", None),
            ("gitlab.com/foo/bar//.yaml@/some/ref", None),
            ("gitlab.com/foo/bar//somedir/.yaml@/some/ref", None),
        ];

        for (input, expected) in test_cases {
            let result = super::extract_workflow_filepath(input);
            assert_eq!(result, expected, "Input: {input}");
        }
    }

    #[test]
    fn test_extract_workflow_filename_redos() {
        let _ = super::extract_workflow_filepath(
            &(".yml@//".repeat(200_000_000) + ".yml@/\n//\x00.yml@y"),
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::extract_workflow_filepath;
    use hegel::generators as gs;
    use hegel::{Generator, TestCase};

    /// Strings assembled from the tokens that drive the parser's branches
    /// (`//`, `@`, extensions, slashes, NUL/newlines) interleaved with arbitrary
    /// text. Any input that does not parse must still terminate and return None.
    fn adversarial() -> impl Generator<String> {
        let tokens: Vec<String> = [
            "//", "@", ".yml", ".yaml", "/", ".", "a", "gitlab.com/foo/bar", "\0", "\n",
            ".wrongsuffix", "refs/heads/main",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        hegel::compose!(|tc| {
            let parts = tc.draw(gs::vecs(gs::sampled_from(tokens.clone())).max_size(20));
            let mut s = parts.join("");
            // Occasionally mix in fully arbitrary text too.
            if tc.draw(gs::booleans()) {
                s.push_str(&tc.draw(gs::text().max_size(16)));
            }
            s
        })
    }

    #[hegel::test(test_cases = 3000)]
    fn prop_never_panics_and_output_is_well_formed(tc: TestCase) {
        let input = tc.draw(adversarial());
        // The call must always terminate (ReDoS-safety) and never panic.
        if let Some(filepath) = extract_workflow_filepath(&input) {
            // Postconditions guaranteed by the parser for any accepted input.
            assert!(
                filepath.ends_with(".yml") || filepath.ends_with(".yaml"),
                "accepted {filepath:?} without a yaml extension"
            );
            let basename = filepath.rsplit('/').next().unwrap();
            assert!(
                basename != ".yml" && basename != ".yaml",
                "accepted bare-extension basename in {filepath:?}"
            );
            assert!(input.contains("//"), "accepted input without `//`: {input:?}");
            assert!(
                input.contains(filepath),
                "result {filepath:?} is not a slice of the input"
            );
        }
    }

    /// Builds a syntactically valid `ci_config_ref_uri` and asserts the parser
    /// extracts exactly the filepath that was embedded.
    #[hegel::test(test_cases = 2000)]
    fn prop_extracts_embedded_filepath(tc: TestCase) {
        // Segment chars exclude `/` and `@` so the only separators are the ones
        // we insert; includes `.` so multi-dot names are exercised.
        const SEG: &str = "abcXYZ012-_.";

        let result = tc.draw(hegel::compose!(|tc| {
            let n_dirs = tc.draw(gs::integers::<usize>().min_value(0).max_value(3));
            let mut segments = Vec::new();
            for _ in 0..n_dirs {
                segments.push(tc.draw(gs::text().alphabet(SEG).min_size(1).max_size(6)));
            }
            let stem = tc.draw(gs::text().alphabet(SEG).min_size(1).max_size(6));
            let ext = tc.draw(gs::sampled_from(vec![".yml", ".yaml"]));
            segments.push(format!("{stem}{ext}"));
            let filepath = segments.join("/");

            // The ref part must contain no `@` so the last `@` is our separator.
            let git_ref = tc.draw(gs::text().alphabet("abc/0123-_ ").max_size(12));
            let input = format!("gitlab.com/foo/bar//{filepath}@{git_ref}");
            (input, filepath)
        }));

        let (input, expected) = result;
        assert_eq!(
            extract_workflow_filepath(&input),
            Some(expected.as_str()),
            "input = {input:?}"
        );
    }
}
