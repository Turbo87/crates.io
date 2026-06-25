//! Property tests for the GitHub and GitLab Trusted Publishing field
//! validators. Each validator is checked against an independent reference
//! predicate (mirroring the documented regex/rules) on a shared fuzz generator,
//! which both pins the accept/reject behaviour and proves the validators never
//! panic on arbitrary input.

use crate::github::validation as gh;
use crate::gitlab::validation as gl;
use hegel::generators as gs;
use hegel::{Generator, TestCase};

const MAX: usize = 255;

fn within_len(s: &str) -> bool {
    !s.is_empty() && s.len() <= MAX
}

// --- GitHub reference predicates -------------------------------------------

fn gh_owner_ok(s: &str) -> bool {
    if !within_len(s) {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    first.is_ascii_alphanumeric() && chars.all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn gh_repo_ok(s: &str) -> bool {
    within_len(s)
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

fn gh_workflow_ok(s: &str) -> bool {
    within_len(s) && (s.ends_with(".yml") || s.ends_with(".yaml")) && !s.contains('/')
}

fn gh_env_ok(s: &str) -> bool {
    within_len(s)
        && !s.starts_with(' ')
        && !s.ends_with(' ')
        && !s.chars().any(|c| {
            let u = c as u32;
            u <= 0x1F || u == 0x7F || matches!(c, '\'' | '"' | '`' | ',' | ';' | '\\')
        })
}

// --- GitLab reference predicates -------------------------------------------

/// Mirrors `^[a-zA-Z0-9](?:[a-zA-Z0-9_.\-/]*[a-zA-Z0-9])?$` (slash optional).
fn gitlab_ident_ok(s: &str, allow_slash: bool) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    if n == 0 || !chars[0].is_ascii_alphanumeric() {
        return false;
    }
    if n == 1 {
        return true;
    }
    if !chars[n - 1].is_ascii_alphanumeric() {
        return false;
    }
    chars[1..n - 1].iter().all(|&c| {
        c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' || (allow_slash && c == '/')
    })
}

fn gl_namespace_ok(s: &str) -> bool {
    within_len(s) && !s.ends_with(".atom") && !s.ends_with(".git") && gitlab_ident_ok(s, true)
}

fn gl_project_ok(s: &str) -> bool {
    within_len(s) && !s.ends_with(".atom") && !s.ends_with(".git") && gitlab_ident_ok(s, false)
}

fn gl_workflow_ok(s: &str) -> bool {
    within_len(s)
        && !s.starts_with('/')
        && !s.ends_with('/')
        && (s.ends_with(".yml") || s.ends_with(".yaml"))
}

fn gl_env_ok(s: &str) -> bool {
    within_len(s)
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || c == ' '
                || c == '-'
                || c == '_'
                || c == '/'
                || c == '$'
                || c == '{'
                || c == '}'
        })
}

// --- Generator -------------------------------------------------------------

fn fuzzy() -> impl hegel::Generator<String> {
    // Covers valid chars for every validator plus chars each one rejects, NUL/
    // control/tab and a non-ASCII char, and an all-`x` branch crossing the
    // 255-byte length limit. A handful of workflow-shaped paths exercise the
    // `.yml`/`.yaml` suffix and slash rules.
    const ALPHABET: &str = "abAB01-_./ @+$#{}'\"`,;\\\t京";
    let workflow_samples: Vec<String> = [
        "publish.yml",
        ".gitlab-ci.yaml",
        "ci/publish.yml",
        "/x.yml",
        "a/",
        "no-suffix",
        "deep/dir/x.yaml",
        "x.yml",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    gs::one_of([
        gs::text().alphabet(ALPHABET).max_size(24).boxed(),
        gs::text().max_size(8).boxed(),
        gs::text().alphabet("x").min_size(250).max_size(260).boxed(),
        gs::sampled_from(workflow_samples).boxed(),
    ])
}

// --- GitHub properties -----------------------------------------------------

#[hegel::test(test_cases = 2000)]
fn prop_github_owner(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(gh::validate_owner(&s).is_ok(), gh_owner_ok(&s), "{s:?}");
}

#[hegel::test(test_cases = 2000)]
fn prop_github_repo(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(gh::validate_repo(&s).is_ok(), gh_repo_ok(&s), "{s:?}");
}

#[hegel::test(test_cases = 2000)]
fn prop_github_workflow_filename(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(
        gh::validate_workflow_filename(&s).is_ok(),
        gh_workflow_ok(&s),
        "{s:?}"
    );
}

#[hegel::test(test_cases = 2000)]
fn prop_github_environment(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(
        gh::validate_environment(&s).is_ok(),
        gh_env_ok(&s),
        "{s:?}"
    );
}

// --- GitLab properties -----------------------------------------------------

#[hegel::test(test_cases = 2000)]
fn prop_gitlab_namespace(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(
        gl::validate_namespace(&s).is_ok(),
        gl_namespace_ok(&s),
        "{s:?}"
    );
}

#[hegel::test(test_cases = 2000)]
fn prop_gitlab_project(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(gl::validate_project(&s).is_ok(), gl_project_ok(&s), "{s:?}");
}

#[hegel::test(test_cases = 2000)]
fn prop_gitlab_workflow_filepath(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(
        gl::validate_workflow_filepath(&s).is_ok(),
        gl_workflow_ok(&s),
        "{s:?}"
    );
}

#[hegel::test(test_cases = 2000)]
fn prop_gitlab_environment(tc: TestCase) {
    let s = tc.draw(fuzzy());
    assert_eq!(
        gl::validate_environment(&s).is_ok(),
        gl_env_ok(&s),
        "{s:?}"
    );
}
