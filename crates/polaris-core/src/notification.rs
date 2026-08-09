use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "desktop-bindings", derive(ts_rs::TS))]
pub struct NotificationPolicy {
    pub state_gate_passed: bool,
    pub dominant_state: Option<String>,
    pub suppress_non_error: bool,
}

pub fn notification_policy(conn: &Connection) -> Result<NotificationPolicy> {
    let payload: Option<String> = conn
        .query_row(
            "SELECT payload_json
             FROM behavior_events
             WHERE type='mental_state'
             ORDER BY julianday(at) DESC, at DESC, id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(payload) = payload else {
        return Ok(NotificationPolicy {
            state_gate_passed: false,
            dominant_state: None,
            suppress_non_error: false,
        });
    };
    let value: serde_json::Value = serde_json::from_str(&payload)?;
    let state_gate_passed = value
        .get("strategy_enabled")
        .and_then(serde_json::Value::as_bool)
        == Some(true);
    let dominant_state = value
        .get("dominant_state")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let suppress_non_error = state_gate_passed && dominant_state.as_deref() == Some("flow");

    Ok(NotificationPolicy {
        state_gate_passed,
        dominant_state,
        suppress_non_error,
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::notification_policy;
    use crate::db::migrate;

    fn connection_with_state(payload: Option<&str>) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        migrate(&connection).unwrap();
        if let Some(payload) = payload {
            connection
                .execute(
                    "INSERT INTO behavior_events(id, at, type, payload_json)
                     VALUES('state-1', '2026-08-09T09:00:00Z', 'mental_state', ?1)",
                    [payload],
                )
                .unwrap();
        }
        connection
    }

    #[test]
    fn missing_or_ungated_state_never_suppresses_notifications() {
        let missing = notification_policy(&connection_with_state(None)).unwrap();
        assert!(!missing.state_gate_passed);
        assert!(!missing.suppress_non_error);

        let ungated = notification_policy(&connection_with_state(Some(
            r#"{"strategy_enabled":false,"dominant_state":"flow"}"#,
        )))
        .unwrap();
        assert_eq!(ungated.dominant_state.as_deref(), Some("flow"));
        assert!(!ungated.state_gate_passed);
        assert!(!ungated.suppress_non_error);
    }

    #[test]
    fn only_gated_flow_suppresses_non_error_notifications() {
        let policy = notification_policy(&connection_with_state(Some(
            r#"{"strategy_enabled":true,"dominant_state":"flow"}"#,
        )))
        .unwrap();

        assert!(policy.state_gate_passed);
        assert_eq!(policy.dominant_state.as_deref(), Some("flow"));
        assert!(policy.suppress_non_error);
    }
}
