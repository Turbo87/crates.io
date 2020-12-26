use crate::{RequestHelper, TestApp};

use conduit::StatusCode;
use serde_json::Value;
use std::env;

static URL: &str = "/api/private/admin/pager-duty";

#[test]
fn admin_can_send_event() {
    let (_, _, user) = TestApp::with_proxy().with_user();

    env::set_var("ADMIN_TEAM_ORG_ID", "13");
    env::set_var("ADMIN_TEAM_ID", "42");

    let payload = json!({ "type": "trigger", "incidentKey": "badc0ffee", "description": "test" });
    let request = user.post::<Value>(URL, payload.to_string().as_bytes());
    request.assert_status(StatusCode::OK);
}

#[test]
fn regular_user_is_rejected() {
    let (_, _, user) = TestApp::with_proxy().with_user();

    env::set_var("ADMIN_TEAM_ORG_ID", "13");
    env::set_var("ADMIN_TEAM_ID", "42");

    let payload = json!({ "type": "trigger", "incidentKey": "badc0ffee", "description": "test" });
    let request = user.post::<Value>(URL, payload.to_string().as_bytes());
    request.assert_status(StatusCode::FORBIDDEN);
}

#[test]
fn unauthenticated_user_is_rejected() {
    let (_, anon) = TestApp::with_proxy().empty();

    let payload = json!({ "type": "trigger", "incidentKey": "badc0ffee", "description": "test" });
    let request = anon.post::<Value>(URL, payload.to_string().as_bytes());
    request.assert_status(StatusCode::FORBIDDEN);
}
