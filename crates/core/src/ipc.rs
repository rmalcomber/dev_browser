//! IPC v0: the messages the trusted shell may send and what the app answers. Core decides, the host only carries the bytes
//! (planning/m1-plan.md, section 4; decisions.md D2).
//!
//! Wire shape: a message is a name and a text payload (JSON from M1.2 on). v0 has one request, `ping`, answered by `pong` with the
//! same payload, so both directions of the channel can be proved end to end.

/// The name of every message the shell may send. Anything else is refused, and the host logs the refusal.
pub const PING: &str = "ping";
pub const PONG: &str = "pong";

/// Longest payload accepted. Bounds what a compromised or buggy sender can make the browser process allocate and log.
pub const MAX_PAYLOAD_BYTES: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    pub name: &'static str,
    pub payload: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    UnknownMessage(String),
    PayloadTooLarge(usize),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::UnknownMessage(n) => write!(f, "unknown message {n:?}"),
            Refused::PayloadTooLarge(n) => write!(
                f,
                "payload of {n} bytes is over the {MAX_PAYLOAD_BYTES}-byte limit"
            ),
        }
    }
}

/// Decide what to do with one message from the shell.
pub fn handle(name: &str, payload: &str) -> Result<Reply, Refused> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(Refused::PayloadTooLarge(payload.len()));
    }
    match name {
        PING => Ok(Reply {
            name: PONG,
            payload: payload.to_string(),
        }),
        other => Err(Refused::UnknownMessage(other.chars().take(64).collect())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_is_answered_with_pong_and_the_same_payload() {
        assert_eq!(
            handle("ping", "hello"),
            Ok(Reply {
                name: "pong",
                payload: "hello".into()
            })
        );
        assert_eq!(
            handle("ping", ""),
            Ok(Reply {
                name: "pong",
                payload: String::new()
            })
        );
    }

    #[test]
    fn unknown_messages_are_refused_not_ignored() {
        assert_eq!(
            handle("delete-everything", ""),
            Err(Refused::UnknownMessage("delete-everything".into()))
        );
        assert_eq!(handle("", ""), Err(Refused::UnknownMessage(String::new())));
        // a hostile name cannot bloat the log line
        let long = "x".repeat(10_000);
        match handle(&long, "") {
            Err(Refused::UnknownMessage(n)) => assert_eq!(n.len(), 64),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn oversized_payloads_are_refused() {
        let big = "a".repeat(MAX_PAYLOAD_BYTES + 1);
        assert_eq!(
            handle("ping", &big),
            Err(Refused::PayloadTooLarge(MAX_PAYLOAD_BYTES + 1))
        );
        assert!(handle("ping", &"a".repeat(MAX_PAYLOAD_BYTES)).is_ok());
    }
}
