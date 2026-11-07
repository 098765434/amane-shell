//! What a Bluetooth failure says: amane reports its kind and BlueZ's own
//! words, and these are the shell's.

use amane::{ErrorKind, ServiceError};

/// A few words for a row in a device list.
pub fn label(failure: &ServiceError) -> &'static str {
    match failure.kind {
        ErrorKind::Unreachable => "Not reachable",
        ErrorKind::AuthenticationFailed => "Pairing failed",
        ErrorKind::Rejected => "Declined",
        ErrorKind::Canceled => "Canceled",
        ErrorKind::TimedOut => "Timed out",
        ErrorKind::NotReady => "Bluetooth is off",
        ErrorKind::Blocked => "Blocked",
        ErrorKind::NotSupported => "Not supported",
        ErrorKind::Gone => "Not found",
        _ => "Couldn't connect",
    }
}

/// A sentence for where there's room.
pub fn description(failure: &ServiceError) -> &str {
    match failure.kind {
        ErrorKind::Unreachable => "Couldn't connect. Make sure it's on and nearby.",
        ErrorKind::AuthenticationFailed => "Pairing failed. The codes may not have matched.",
        ErrorKind::Rejected => "The pairing was declined.",
        ErrorKind::Canceled => "The pairing was canceled.",
        ErrorKind::TimedOut => "The device didn't answer in time.",
        ErrorKind::NotReady => "Bluetooth is off.",
        ErrorKind::Blocked => "Bluetooth is blocked by airplane mode or a hardware switch.",
        ErrorKind::NotSupported => "This device has nothing this computer can connect to.",
        ErrorKind::Gone => "The device is no longer nearby.",
        _ if !failure.message.is_empty() => &failure.message,
        _ => "Something went wrong.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_label_fits_a_row() {
        let kinds = [
            ErrorKind::Unreachable,
            ErrorKind::AuthenticationFailed,
            ErrorKind::Rejected,
            ErrorKind::Canceled,
            ErrorKind::TimedOut,
            ErrorKind::NotReady,
            ErrorKind::Blocked,
            ErrorKind::NotSupported,
            ErrorKind::Gone,
            ErrorKind::PermissionDenied,
            ErrorKind::Unavailable,
            ErrorKind::Failed,
        ];
        for kind in kinds {
            let label = label(&ServiceError::new(kind, ""));
            assert!(label.chars().count() <= 16, "{label}");
        }
    }

    #[test]
    fn an_unknown_failure_says_what_bluez_said() {
        let failure = ServiceError::new(ErrorKind::Failed, "br-connection-unknown-thing");
        assert_eq!(description(&failure), "br-connection-unknown-thing");
        assert_eq!(
            description(&ServiceError::new(ErrorKind::Failed, "")),
            "Something went wrong."
        );
    }
}
