//! IPC wire types shared by the backend and the host: what a reply is, and why a message is refused. The messages themselves and what
//! they do live in `backend` (core decides, the host only carries bytes and executes effects; planning/m1-plan.md, section 4).
//!
//! Wire shape: a message is a name and a text payload. From M1.2 on the payload is JSON, except `ping`, whose payload is any text.
//! A **refusal** is a protocol violation (unknown name, oversized or malformed payload): the host logs it and sends nothing back.
//! A **user error** (an invalid workspace name, deleting the last workspace) is not a refusal: it is answered with an `error` reply.

/// Longest payload accepted. Bounds what a compromised or buggy sender can make the browser process allocate, parse and log.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Reply {
    pub name: &'static str,
    pub payload: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    UnknownMessage(String),
    PayloadTooLarge(usize),
    /// The payload is not the JSON this message needs. The text says what was wrong (never echoes the payload).
    BadPayload {
        message: String,
        why: String,
    },
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::UnknownMessage(n) => write!(f, "unknown message {n:?}"),
            Refused::PayloadTooLarge(n) => write!(
                f,
                "payload of {n} bytes is over the {MAX_PAYLOAD_BYTES}-byte limit"
            ),
            Refused::BadPayload { message, why } => {
                write!(f, "bad payload for {message:?}: {why}")
            }
        }
    }
}

impl std::error::Error for Refused {}
