use std::os::fd::RawFd;

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, INTERACTION_LEASE_FD_ENV, InteractionLease,
    acquire_unix_interaction_lease, adopt_inherited_unix_interaction_lease,
};

use super::acquired_lease::AcquiredLease;
use super::owned_lease::OwnedLease;

const INVALID_FD: &str = "Inherited interaction lease FD must be a nonnegative integer";

/// Takes the interaction lock, or adopts the lease the parent passed down when the inherited-descriptor
/// variable is set.
pub(crate) fn acquire(deadline: Deadline) -> Result<InteractionLease, AdapterError> {
    acquire_with_origin(deadline).map(AcquiredLease::into_lease)
}

/// `acquire`, saying whether the lease was taken or adopted, so work that needs the lock to itself can
/// refuse an adopted one.
pub(crate) fn acquire_with_origin(deadline: Deadline) -> Result<AcquiredLease, AdapterError> {
    let Some(raw) = std::env::var_os(INTERACTION_LEASE_FD_ENV) else {
        return acquire_unix_interaction_lease(deadline)
            .map(|lease| AcquiredLease::Owned(OwnedLease::new(lease)));
    };
    let raw = raw.into_string().map_err(|_| {
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "Inherited interaction lease FD must be valid UTF-8",
        )
    })?;
    let fd = parse_fd(&raw)?;
    adopt_inherited_unix_interaction_lease(fd, deadline).map(AcquiredLease::Adopted)
}

fn parse_fd(raw: &str) -> Result<RawFd, AdapterError> {
    match raw.parse::<RawFd>() {
        Ok(fd) if fd >= 0 => Ok(fd),
        _ => Err(AdapterError::new(ErrorCode::InvalidArgs, INVALID_FD)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inherited_descriptor_must_be_a_nonnegative_integer() {
        assert_eq!(parse_fd("7").expect("a plain descriptor parses"), 7);
        for bad in ["-1", "x", ""] {
            let error = parse_fd(bad).expect_err("a malformed descriptor must be refused");
            assert_eq!(error.code, ErrorCode::InvalidArgs);
        }
    }
}
