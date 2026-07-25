//! Shared, dependency-free security primitives usable by any crate that
//! depends on `grumps-core` — including `grumps-messaging`, which must stay
//! dependency-light and pure (no worker/runtime types). Nothing here reaches
//! outside `std`.

/// Constant-time byte comparison. Use this instead of `==` for anything that
/// compares a secret (webhook tokens, CSRF tokens, OTP codes, HMAC digests
/// encoded as hex) against attacker-controlled input — a naive `==` short-
/// circuits on the first mismatched byte, which leaks the correct prefix
/// length through response timing.
///
/// Length is not secret (an attacker can always trivially discover it by
/// trying inputs of different lengths), so returning early on a length
/// mismatch does not reintroduce a timing side-channel on the secret's
/// content.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_identical_bytes() {
        assert!(constant_time_eq(b"s3cr3t", b"s3cr3t"));
    }

    #[test]
    fn rejects_different_case() {
        assert!(!constant_time_eq(b"s3cr3t", b"S3cr3t"));
    }

    #[test]
    fn rejects_different_length() {
        assert!(!constant_time_eq(b"s3cr3t", b"s3cr3t-longer"));
    }

    #[test]
    fn empty_never_matches_nonempty() {
        assert!(!constant_time_eq(b"", b"s3cr3t"));
    }

    #[test]
    fn empty_matches_empty() {
        assert!(constant_time_eq(b"", b""));
    }
}
