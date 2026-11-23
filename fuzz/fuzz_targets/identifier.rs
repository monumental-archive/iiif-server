// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Identifier decoding is a named security component: never panics, and
//! nothing that decodes successfully contains a traversal shape.
#![no_main]

use core::str::from_utf8;

use iiif_core::ident::Identifier;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(input) = from_utf8(data) else {
        return;
    };
    if let Ok(id) = Identifier::decode(input) {
        let path = id.as_path();
        assert!(!path.starts_with('/'), "absolute path from {input:?}");
        assert!(!path.contains('\\'), "backslash in path from {input:?}");
        for segment in path.split('/') {
            assert!(!segment.is_empty(), "empty segment from {input:?}");
            assert_ne!(segment, ".", "dot segment from {input:?}");
            assert_ne!(segment, "..", "traversal segment from {input:?}");
        }
        // Re-encoding must round-trip.
        let re = Identifier::decode(&id.encoded());
        assert_eq!(re, Ok(id), "round-trip mismatch for {input:?}");
    }
});
