#![doc = include_str!("../README.md")]

pub const MAX_NAME_LENGTH: usize = 64;

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum InvalidFeature {
    #[error("feature cannot be empty")]
    Empty,
    #[error(
        "invalid character `{0}` in feature `{1}`, the first character must be \
        a Unicode XID start character or digit (most letters or `_` or `0` to \
        `9`)"
    )]
    Start(char, String),
    #[error(
        "invalid character `{0}` in feature `{1}`, characters must be Unicode \
        XID characters, `+`, `-`, or `.` (numbers, `+`, `-`, `_`, `.`, or most \
        letters)"
    )]
    Char(char, String),
    #[error(transparent)]
    DependencyName(#[from] InvalidDependencyName),
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum InvalidCrateName {
    #[error("the {what} name `{name}` is too long (max {MAX_NAME_LENGTH} characters)")]
    TooLong { what: String, name: String },
    #[error("{what} name cannot be empty")]
    Empty { what: String },
    #[error(
        "the name `{name}` cannot be used as a {what} name, \
        the name cannot start with a digit"
    )]
    StartWithDigit { what: String, name: String },
    #[error(
        "invalid character `{first_char}` in {what} name: `{name}`, \
        the first character must be an ASCII character"
    )]
    Start {
        first_char: char,
        what: String,
        name: String,
    },
    #[error(
        "invalid character `{ch}` in {what} name: `{name}`, \
        characters must be an ASCII alphanumeric characters, `-`, or `_`"
    )]
    Char {
        ch: char,
        what: String,
        name: String,
    },
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum InvalidDependencyName {
    #[error("the dependency name `{0}` is too long (max {MAX_NAME_LENGTH} characters)")]
    TooLong(String),
    #[error("dependency name cannot be empty")]
    Empty,
    #[error(
        "the name `{0}` cannot be used as a dependency name, \
        the name cannot start with a digit"
    )]
    StartWithDigit(String),
    #[error(
        "invalid character `{0}` in dependency name: `{1}`, \
        the first character must be an ASCII character, or `_`"
    )]
    Start(char, String),
    #[error(
        "invalid character `{0}` in dependency name: `{1}`, \
        characters must be an ASCII alphanumeric characters, `-`, or `_`"
    )]
    Char(char, String),
}

// Validates the name is a valid crate name.
// This is also used for validating the name of dependencies.
// So the `for_what` parameter is used to indicate what the name is used for.
// It can be "crate" or "dependency".
pub fn validate_crate_name(for_what: &str, name: &str) -> Result<(), InvalidCrateName> {
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(InvalidCrateName::TooLong {
            what: for_what.into(),
            name: name.into(),
        });
    }
    validate_create_ident(for_what, name)
}

// Checks that the name is a valid crate name.
// 1. The name must be non-empty.
// 2. The first character must be an ASCII character.
// 3. The remaining characters must be ASCII alphanumerics or `-` or `_`.
// Note: This differs from `valid_dependency_name`, which allows `_` as the first character.
fn validate_create_ident(for_what: &str, name: &str) -> Result<(), InvalidCrateName> {
    if name.is_empty() {
        return Err(InvalidCrateName::Empty {
            what: for_what.into(),
        });
    }
    let mut chars = name.chars();
    if let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            return Err(InvalidCrateName::StartWithDigit {
                what: for_what.into(),
                name: name.into(),
            });
        }
        if !ch.is_ascii_alphabetic() {
            return Err(InvalidCrateName::Start {
                first_char: ch,
                what: for_what.into(),
                name: name.into(),
            });
        }
    }

    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            return Err(InvalidCrateName::Char {
                ch,
                what: for_what.into(),
                name: name.into(),
            });
        }
    }

    Ok(())
}

pub fn validate_dependency_name(name: &str) -> Result<(), InvalidDependencyName> {
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(InvalidDependencyName::TooLong(name.into()));
    }
    validate_dependency_ident(name)
}

// Checks that the name is a valid dependency name.
// 1. The name must be non-empty.
// 2. The first character must be an ASCII character or `_`.
// 3. The remaining characters must be ASCII alphanumerics or `-` or `_`.
fn validate_dependency_ident(name: &str) -> Result<(), InvalidDependencyName> {
    if name.is_empty() {
        return Err(InvalidDependencyName::Empty);
    }
    let mut chars = name.chars();
    if let Some(ch) = chars.next() {
        if ch.is_ascii_digit() {
            return Err(InvalidDependencyName::StartWithDigit(name.into()));
        }
        if !(ch.is_ascii_alphabetic() || ch == '_') {
            return Err(InvalidDependencyName::Start(ch, name.into()));
        }
    }

    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            return Err(InvalidDependencyName::Char(ch, name.into()));
        }
    }

    Ok(())
}

/// Validates the THIS parts of `features = ["THIS", "and/THIS", "dep:THIS", "dep?/THIS"]`.
/// 1. The name must be non-empty.
/// 2. The first character must be a Unicode XID start character, `_`, or a digit.
/// 3. The remaining characters must be Unicode XID characters, `_`, `+`, `-`, or `.`.
pub fn validate_feature_name(name: &str) -> Result<(), InvalidFeature> {
    if name.is_empty() {
        return Err(InvalidFeature::Empty);
    }
    let mut chars = name.chars();
    if let Some(ch) = chars.next()
        && !(unicode_xid::UnicodeXID::is_xid_start(ch) || ch == '_' || ch.is_ascii_digit())
    {
        return Err(InvalidFeature::Start(ch, name.into()));
    }
    for ch in chars {
        if !(unicode_xid::UnicodeXID::is_xid_continue(ch) || ch == '+' || ch == '-' || ch == '.') {
            return Err(InvalidFeature::Char(ch, name.into()));
        }
    }

    Ok(())
}

/// Validates a whole feature string, `features = ["THIS", "and/THIS", "dep:THIS", "dep?/THIS"]`.
pub fn validate_feature(name: &str) -> Result<(), InvalidFeature> {
    if let Some((dep, dep_feat)) = name.split_once('/') {
        let dep = dep.strip_suffix('?').unwrap_or(dep);
        validate_dependency_name(dep)?;
        validate_feature_name(dep_feat)
    } else if let Some((_, dep)) = name.split_once("dep:") {
        validate_dependency_name(dep)?;
        Ok(())
    } else {
        validate_feature_name(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use claims::{assert_err_eq, assert_ok};

    #[test]
    fn test_validate_crate_name() {
        use super::{InvalidCrateName, MAX_NAME_LENGTH};

        assert_ok!(validate_crate_name("crate", "foo"));
        assert_err_eq!(
            validate_crate_name("crate", "京"),
            InvalidCrateName::Start {
                first_char: '京',
                what: "crate".into(),
                name: "京".into()
            }
        );
        assert_err_eq!(
            validate_crate_name("crate", ""),
            InvalidCrateName::Empty {
                what: "crate".into()
            }
        );
        assert_err_eq!(
            validate_crate_name("crate", "💝"),
            InvalidCrateName::Start {
                first_char: '💝',
                what: "crate".into(),
                name: "💝".into()
            }
        );
        assert_ok!(validate_crate_name("crate", "foo_underscore"));
        assert_ok!(validate_crate_name("crate", "foo-dash"));
        assert_err_eq!(
            validate_crate_name("crate", "foo+plus"),
            InvalidCrateName::Char {
                ch: '+',
                what: "crate".into(),
                name: "foo+plus".into()
            }
        );
        assert_err_eq!(
            validate_crate_name("crate", "_foo"),
            InvalidCrateName::Start {
                first_char: '_',
                what: "crate".into(),
                name: "_foo".into()
            }
        );
        assert_err_eq!(
            validate_crate_name("crate", "-foo"),
            InvalidCrateName::Start {
                first_char: '-',
                what: "crate".into(),
                name: "-foo".into()
            }
        );
        assert_err_eq!(
            validate_crate_name("crate", "123"),
            InvalidCrateName::StartWithDigit {
                what: "crate".into(),
                name: "123".into()
            }
        );
        assert_err_eq!(
            validate_crate_name("crate", "o".repeat(MAX_NAME_LENGTH + 1).as_str()),
            InvalidCrateName::TooLong {
                what: "crate".into(),
                name: "o".repeat(MAX_NAME_LENGTH + 1).as_str().into()
            }
        );
    }

    #[test]
    fn test_validate_dependency_name() {
        use super::{InvalidDependencyName, MAX_NAME_LENGTH};

        assert_ok!(validate_dependency_name("foo"));
        assert_err_eq!(
            validate_dependency_name("京"),
            InvalidDependencyName::Start('京', "京".into())
        );
        assert_err_eq!(validate_dependency_name(""), InvalidDependencyName::Empty);
        assert_err_eq!(
            validate_dependency_name("💝"),
            InvalidDependencyName::Start('💝', "💝".into())
        );
        assert_ok!(validate_dependency_name("foo_underscore"));
        assert_ok!(validate_dependency_name("foo-dash"));
        assert_err_eq!(
            validate_dependency_name("foo+plus"),
            InvalidDependencyName::Char('+', "foo+plus".into())
        );
        // Starting with an underscore is a valid dependency name.
        assert_ok!(validate_dependency_name("_foo"));
        assert_err_eq!(
            validate_dependency_name("-foo"),
            InvalidDependencyName::Start('-', "-foo".into())
        );
        assert_err_eq!(
            validate_dependency_name("o".repeat(MAX_NAME_LENGTH + 1).as_str()),
            InvalidDependencyName::TooLong("o".repeat(MAX_NAME_LENGTH + 1).as_str().into())
        );
    }

    #[test]
    fn test_validate_feature_names() {
        use super::InvalidDependencyName;
        use super::InvalidFeature;

        assert_ok!(validate_feature("foo"));
        assert_ok!(validate_feature("1foo"));
        assert_ok!(validate_feature("_foo"));
        assert_ok!(validate_feature("_foo-_+.1"));
        assert_ok!(validate_feature("_foo-_+.1"));
        assert_err_eq!(validate_feature(""), InvalidFeature::Empty);
        assert_err_eq!(validate_feature("/"), InvalidDependencyName::Empty.into());
        assert_err_eq!(
            validate_feature("%/%"),
            InvalidDependencyName::Start('%', "%".into()).into()
        );
        assert_ok!(validate_feature("a/a"));
        assert_ok!(validate_feature("32-column-tables"));
        assert_ok!(validate_feature("c++20"));
        assert_ok!(validate_feature("krate/c++20"));
        assert_err_eq!(
            validate_feature("c++20/wow"),
            InvalidDependencyName::Char('+', "c++20".into()).into()
        );
        assert_ok!(validate_feature("foo?/bar"));
        assert_ok!(validate_feature("dep:foo"));
        assert_err_eq!(
            validate_feature("dep:foo?/bar"),
            InvalidDependencyName::Char(':', "dep:foo".into()).into()
        );
        assert_err_eq!(
            validate_feature("foo/?bar"),
            InvalidFeature::Start('?', "?bar".into())
        );
        assert_err_eq!(
            validate_feature("foo?bar"),
            InvalidFeature::Char('?', "foo?bar".into())
        );
        assert_ok!(validate_feature("bar.web"));
        assert_ok!(validate_feature("foo/bar.web"));
        assert_err_eq!(
            validate_feature("dep:0foo"),
            InvalidDependencyName::StartWithDigit("0foo".into()).into()
        );
        assert_err_eq!(
            validate_feature("0foo?/bar.web"),
            InvalidDependencyName::StartWithDigit("0foo".into()).into()
        );
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use hegel::generators as gs;
    use hegel::{Generator, TestCase};
    use unicode_xid::UnicodeXID;

    const LETTERS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    /// Characters allowed in the tail of a crate/dependency name.
    const NAME_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";

    // Independent reference reimplementations of the accept/reject rules. The
    // properties below assert that the real validators agree with these on
    // arbitrary input, which both pins down the documented behaviour and proves
    // the validators never panic (they run on every drawn string).

    fn ref_crate_name(name: &str) -> bool {
        if name.is_empty() || name.chars().count() > MAX_NAME_LENGTH {
            return false;
        }
        let mut chars = name.chars();
        let first = chars.next().unwrap();
        first.is_ascii_alphabetic() && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    fn ref_dependency_name(name: &str) -> bool {
        if name.is_empty() || name.chars().count() > MAX_NAME_LENGTH {
            return false;
        }
        let mut chars = name.chars();
        let first = chars.next().unwrap();
        (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }

    fn ref_feature_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut chars = name.chars();
        let first = chars.next().unwrap();
        (UnicodeXID::is_xid_start(first) || first == '_' || first.is_ascii_digit())
            && chars.all(|c| UnicodeXID::is_xid_continue(c) || c == '+' || c == '-' || c == '.')
    }

    fn ref_feature(name: &str) -> bool {
        if let Some((dep, dep_feat)) = name.split_once('/') {
            let dep = dep.strip_suffix('?').unwrap_or(dep);
            ref_dependency_name(dep) && ref_feature_name(dep_feat)
        } else if let Some((_, dep)) = name.split_once("dep:") {
            ref_dependency_name(dep)
        } else {
            ref_feature_name(name)
        }
    }

    /// A mix of arbitrary Unicode and a focused alphabet that frequently lands on
    /// the boundaries that matter (separators, the 64-char length limit, and the
    /// `dep:` / `?/` feature syntax). Drawing from both gives the no-panic /
    /// reference-agreement checks broad and targeted coverage at once.
    fn fuzz_names() -> impl Generator<String> {
        const FUZZ_ALPHABET: &str = "abAB01-_+./?: 京💝\t";
        gs::one_of([
            gs::text().max_size(70).boxed(),
            gs::text().alphabet(FUZZ_ALPHABET).max_size(70).boxed(),
        ])
    }

    #[hegel::test(test_cases = 2000)]
    fn prop_crate_name_matches_reference(tc: TestCase) {
        let name = tc.draw(fuzz_names());
        assert_eq!(
            validate_crate_name("crate", &name).is_ok(),
            ref_crate_name(&name),
            "name = {name:?}"
        );
    }

    #[hegel::test(test_cases = 2000)]
    fn prop_dependency_name_matches_reference(tc: TestCase) {
        let name = tc.draw(fuzz_names());
        assert_eq!(
            validate_dependency_name(&name).is_ok(),
            ref_dependency_name(&name),
            "name = {name:?}"
        );
    }

    #[hegel::test(test_cases = 2000)]
    fn prop_feature_name_matches_reference(tc: TestCase) {
        let name = tc.draw(fuzz_names());
        assert_eq!(
            validate_feature_name(&name).is_ok(),
            ref_feature_name(&name),
            "name = {name:?}"
        );
    }

    #[hegel::test(test_cases = 2000)]
    fn prop_feature_matches_reference(tc: TestCase) {
        let name = tc.draw(fuzz_names());
        assert_eq!(
            validate_feature(&name).is_ok(),
            ref_feature(&name),
            "name = {name:?}"
        );
    }

    #[hegel::test]
    fn prop_crate_name_accepts_constructed_valid(tc: TestCase) {
        let first = tc.draw(gs::text().alphabet(LETTERS).min_size(1).max_size(1));
        let rest = tc.draw(gs::text().alphabet(NAME_CHARS).max_size(MAX_NAME_LENGTH - 1));
        let name = format!("{first}{rest}");
        assert!(validate_crate_name("crate", &name).is_ok(), "name = {name:?}");
    }

    #[hegel::test]
    fn prop_dependency_name_accepts_constructed_valid(tc: TestCase) {
        // Dependency names additionally allow a leading underscore.
        let first = tc.draw(gs::text().alphabet(concat!("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ", "_")).min_size(1).max_size(1));
        let rest = tc.draw(gs::text().alphabet(NAME_CHARS).max_size(MAX_NAME_LENGTH - 1));
        let name = format!("{first}{rest}");
        assert!(validate_dependency_name(&name).is_ok(), "name = {name:?}");
    }
}
