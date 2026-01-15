//! Tag name validation for padz.
//!
//! Valid tags:
//! - Alphanumeric characters, underscores (`_`), and hyphens (`-`)
//! - Must start with a letter
//! - Cannot have consecutive hyphens (`--`)
//! - Cannot end with a hyphen

/// Validates a tag name according to padz tag naming rules.
///
/// # Rules
/// - Must contain only alphanumeric characters, underscores, and hyphens
/// - Must start with a letter (a-z, A-Z)
/// - Cannot contain consecutive hyphens (`--`)
/// - Cannot end with a hyphen (`-`)
///
/// # Examples
/// ```
/// use padzapp::tags::validation::validate_tag_name;
///
/// assert!(validate_tag_name("foo").is_ok());
/// assert!(validate_tag_name("foo-bar").is_ok());
/// assert!(validate_tag_name("foo_bar").is_ok());
/// assert!(validate_tag_name("f7-bar8").is_ok());
///
/// assert!(validate_tag_name("").is_err());
/// assert!(validate_tag_name("-foo").is_err());
/// assert!(validate_tag_name("foo-").is_err());
/// assert!(validate_tag_name("foo--bar").is_err());
/// assert!(validate_tag_name("7foo").is_err());
/// ```
pub fn validate_tag_name(name: &str) -> Result<(), TagValidationError> {
    if name.is_empty() {
        return Err(TagValidationError::Empty);
    }

    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() {
        return Err(TagValidationError::InvalidStart(first_char));
    }

    let last_char = name.chars().last().unwrap();
    if last_char == '-' {
        return Err(TagValidationError::EndsWithHyphen);
    }

    // Check for invalid characters and consecutive hyphens
    let mut prev_was_hyphen = false;
    for ch in name.chars() {
        if !is_valid_tag_char(ch) {
            return Err(TagValidationError::InvalidCharacter(ch));
        }

        if ch == '-' {
            if prev_was_hyphen {
                return Err(TagValidationError::ConsecutiveHyphens);
            }
            prev_was_hyphen = true;
        } else {
            prev_was_hyphen = false;
        }
    }

    Ok(())
}

/// Checks if a character is valid in a tag name.
fn is_valid_tag_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

/// Error type for tag name validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagValidationError {
    /// Tag name is empty
    Empty,
    /// Tag name starts with an invalid character (must start with a letter)
    InvalidStart(char),
    /// Tag name ends with a hyphen
    EndsWithHyphen,
    /// Tag name contains consecutive hyphens
    ConsecutiveHyphens,
    /// Tag name contains an invalid character
    InvalidCharacter(char),
}

impl std::fmt::Display for TagValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TagValidationError::Empty => write!(f, "tag name cannot be empty"),
            TagValidationError::InvalidStart(ch) => {
                write!(f, "tag name must start with a letter, found '{}'", ch)
            }
            TagValidationError::EndsWithHyphen => {
                write!(f, "tag name cannot end with a hyphen")
            }
            TagValidationError::ConsecutiveHyphens => {
                write!(f, "tag name cannot contain consecutive hyphens")
            }
            TagValidationError::InvalidCharacter(ch) => {
                write!(
                    f,
                    "tag name contains invalid character '{}' (only alphanumeric, underscore, and hyphen allowed)",
                    ch
                )
            }
        }
    }
}

impl std::error::Error for TagValidationError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple_tags() {
        assert!(validate_tag_name("foo").is_ok());
        assert!(validate_tag_name("bar").is_ok());
        assert!(validate_tag_name("work").is_ok());
    }

    #[test]
    fn test_valid_tags_with_hyphens() {
        assert!(validate_tag_name("foo-bar").is_ok());
        assert!(validate_tag_name("my-project").is_ok());
        assert!(validate_tag_name("a-b-c").is_ok());
    }

    #[test]
    fn test_valid_tags_with_underscores() {
        assert!(validate_tag_name("foo_bar").is_ok());
        assert!(validate_tag_name("my_project").is_ok());
        assert!(validate_tag_name("a_b_c").is_ok());
    }

    #[test]
    fn test_valid_tags_with_numbers() {
        assert!(validate_tag_name("f7-bar8").is_ok());
        assert!(validate_tag_name("f8-3").is_ok());
        assert!(validate_tag_name("f80-3_x").is_ok());
        assert!(validate_tag_name("project2024").is_ok());
    }

    #[test]
    fn test_valid_mixed_tags() {
        assert!(validate_tag_name("my-project_2024").is_ok());
        assert!(validate_tag_name("foo_bar-baz").is_ok());
    }

    #[test]
    fn test_invalid_empty() {
        assert_eq!(validate_tag_name(""), Err(TagValidationError::Empty));
    }

    #[test]
    fn test_invalid_starts_with_hyphen() {
        assert_eq!(
            validate_tag_name("-foo"),
            Err(TagValidationError::InvalidStart('-'))
        );
    }

    #[test]
    fn test_invalid_starts_with_underscore() {
        assert_eq!(
            validate_tag_name("_foo"),
            Err(TagValidationError::InvalidStart('_'))
        );
    }

    #[test]
    fn test_invalid_starts_with_number() {
        assert_eq!(
            validate_tag_name("7foo"),
            Err(TagValidationError::InvalidStart('7'))
        );
        assert_eq!(
            validate_tag_name("123"),
            Err(TagValidationError::InvalidStart('1'))
        );
    }

    #[test]
    fn test_invalid_ends_with_hyphen() {
        assert_eq!(
            validate_tag_name("foo-"),
            Err(TagValidationError::EndsWithHyphen)
        );
        assert_eq!(
            validate_tag_name("bar-baz-"),
            Err(TagValidationError::EndsWithHyphen)
        );
    }

    #[test]
    fn test_invalid_consecutive_hyphens() {
        assert_eq!(
            validate_tag_name("foo--bar"),
            Err(TagValidationError::ConsecutiveHyphens)
        );
        assert_eq!(
            validate_tag_name("a---b"),
            Err(TagValidationError::ConsecutiveHyphens)
        );
    }

    #[test]
    fn test_invalid_characters() {
        assert_eq!(
            validate_tag_name("foo bar"),
            Err(TagValidationError::InvalidCharacter(' '))
        );
        assert_eq!(
            validate_tag_name("foo.bar"),
            Err(TagValidationError::InvalidCharacter('.'))
        );
        assert_eq!(
            validate_tag_name("foo@bar"),
            Err(TagValidationError::InvalidCharacter('@'))
        );
        assert_eq!(
            validate_tag_name("foo#bar"),
            Err(TagValidationError::InvalidCharacter('#'))
        );
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            TagValidationError::Empty.to_string(),
            "tag name cannot be empty"
        );
        assert_eq!(
            TagValidationError::InvalidStart('-').to_string(),
            "tag name must start with a letter, found '-'"
        );
        assert_eq!(
            TagValidationError::EndsWithHyphen.to_string(),
            "tag name cannot end with a hyphen"
        );
        assert_eq!(
            TagValidationError::ConsecutiveHyphens.to_string(),
            "tag name cannot contain consecutive hyphens"
        );
        assert_eq!(
            TagValidationError::InvalidCharacter('@').to_string(),
            "tag name contains invalid character '@' (only alphanumeric, underscore, and hyphen allowed)"
        );
    }
}
