// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Identifier decoding: the security component gets its own test file.

#![expect(
    clippy::min_ident_chars,
    clippy::missing_panics_doc,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    reason = "test and example code. A panic IS the failure signal here, so \
              `# Panics` sections and assertion messages would describe the \
              mechanism the harness works by; fixtures are indexed and \
              scaled with arithmetic over constants in the file above them; \
              and a `#[test]` at the top level of a `tests/` file is what an \
              integration test IS. The crate under test is held to every \
              one of these."
)]

use iiif_core::ident::{Identifier, IdentifierError};
use proptest::prelude::*;

#[track_caller]
fn ok(raw: &str) -> Identifier {
    Identifier::decode(raw).unwrap()
}

#[track_caller]
fn rejects(raw: &str, why: IdentifierError) {
    assert_eq!(Identifier::decode(raw).unwrap_err(), why, "input {raw:?}");
}

#[test]
fn plain_identifiers() {
    assert_eq!(ok("abcd1234").as_path(), "abcd1234");
    assert_eq!(ok("a-b_c.tif").as_path(), "a-b_c.tif");
    // Encoded slash means a subdirectory path.
    assert_eq!(ok("dir%2Fimage.tif").as_path(), "dir/image.tif");
    assert_eq!(ok("a%2Fb%2Fc").as_path(), "a/b/c");
    // Unencoded non-special characters survive.
    assert_eq!(ok("M%C3%BCnchen.jp2").as_path(), "M\u{fc}nchen.jp2");
}

#[test]
fn dots_that_are_not_traversal() {
    assert_eq!(ok("..foo").as_path(), "..foo");
    assert_eq!(ok("foo..").as_path(), "foo..");
    assert_eq!(ok("f.o.o").as_path(), "f.o.o");
    assert_eq!(ok("a%2F..b").as_path(), "a/..b");
}

#[test]
fn traversal_rejected() {
    rejects("..", IdentifierError::Traversal);
    rejects(".", IdentifierError::Traversal);
    rejects("%2E%2E", IdentifierError::Traversal);
    rejects("..%2Fetc%2Fpasswd", IdentifierError::Traversal);
    rejects("a%2F..%2Fb", IdentifierError::Traversal);
    rejects("a%2F.%2Fb", IdentifierError::Traversal);
    rejects("%2Fabsolute", IdentifierError::Traversal);
    rejects("a%2F%2Fb", IdentifierError::Traversal); // empty segment
    rejects("trailing%2F", IdentifierError::Traversal);
}

#[test]
fn single_decode_pass_only() {
    // %252E is "%2E" after one pass — it must stay literal "%2E", never
    // become ".". If double-decoding ever creeps in, this becomes "." and
    // the traversal check would fire — either way the test fails loudly.
    assert_eq!(ok("%252E%252E").as_path(), "%2E%2E");
    assert_eq!(ok("a%252Fb").as_path(), "a%2Fb");
}

#[test]
fn bad_encodings_rejected() {
    rejects("%", IdentifierError::BadEscape);
    rejects("%2", IdentifierError::BadEscape);
    rejects("%GG", IdentifierError::BadEscape);
    rejects("a%2Gb", IdentifierError::BadEscape);
    rejects("", IdentifierError::Empty);
    rejects("%FF", IdentifierError::NotUtf8);
    rejects("a%00b", IdentifierError::ControlCharacter);
    rejects("a%0Ab", IdentifierError::ControlCharacter);
    rejects("a%7Fb", IdentifierError::ControlCharacter);
    rejects("a%5Cb", IdentifierError::Backslash);
    rejects("a\\b", IdentifierError::Backslash);
}

proptest! {
    /// decode(encode(id)) is the identity for every decodable identifier.
    #[test]
    fn encode_decode_roundtrip(raw in "[a-zA-Z0-9._%/-]{1,64}") {
        // Build via decode of arbitrary raw input; only test when valid.
        if let Ok(id) = Identifier::decode(&raw) {
            let re = Identifier::decode(&id.encoded()).unwrap();
            prop_assert_eq!(re, id);
        }
    }

    /// Decoding never panics on arbitrary input.
    #[test]
    fn decode_never_panics(raw in "\\PC*") {
        drop(Identifier::decode(&raw));
    }

    /// Whatever decodes successfully never contains traversal segments.
    #[test]
    fn no_traversal_survives(raw in "\\PC*") {
        if let Ok(id) = Identifier::decode(&raw) {
            let p = id.as_path();
            prop_assert!(!p.starts_with('/'));
            for seg in p.split('/') {
                prop_assert!(!seg.is_empty());
                prop_assert_ne!(seg, ".");
                prop_assert_ne!(seg, "..");
            }
        }
    }
}
