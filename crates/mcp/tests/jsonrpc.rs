use super::*;
use serde_json::json;

#[test]
fn parses_a_request() {
    let req = Request::parse(r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{"a":1}}"#)
        .expect("valid request");
    assert_eq!(req.id, Some(json!(7)));
    assert_eq!(req.method, "tools/list");
    assert_eq!(req.params, json!({ "a": 1 }));
    assert!(!req.is_notification());
}

#[test]
fn a_message_without_id_is_a_notification() {
    let req = Request::parse(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).unwrap();
    assert!(req.is_notification());
    // A present-but-null id is also a notification (nothing to correlate).
    let req = Request::parse(r#"{"jsonrpc":"2.0","id":null,"method":"x"}"#).unwrap();
    assert!(req.is_notification());
}

#[test]
fn missing_params_defaults_to_null() {
    let req = Request::parse(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#).unwrap();
    assert_eq!(req.params, serde_json::Value::Null);
}

#[test]
fn malformed_json_is_a_parse_error() {
    let err = *Request::parse("{not json").unwrap_err();
    match err.payload {
        Payload::Error { code, .. } => assert_eq!(code, PARSE_ERROR),
        _ => panic!("expected error"),
    }
    assert_eq!(err.id, serde_json::Value::Null);
}

#[test]
fn a_request_without_method_is_invalid() {
    let err = *Request::parse(r#"{"jsonrpc":"2.0","id":3}"#).unwrap_err();
    match err.payload {
        Payload::Error { code, .. } => assert_eq!(code, INVALID_REQUEST),
        _ => panic!("expected error"),
    }
    // The id is echoed so the client can correlate the failure.
    assert_eq!(err.id, json!(3));
}

#[test]
fn a_non_object_message_is_invalid() {
    assert!(Request::parse("[1,2,3]").is_err());
    assert!(Request::parse("42").is_err());
}

#[test]
fn result_and_error_serialize_to_the_envelope() {
    let ok = Response::result(json!(1), json!({ "ok": true })).to_value();
    assert_eq!(ok["jsonrpc"], "2.0");
    assert_eq!(ok["id"], 1);
    assert_eq!(ok["result"], json!({ "ok": true }));
    assert!(ok.get("error").is_none());

    let bad = Response::error(json!(2), METHOD_NOT_FOUND, "nope").to_value();
    assert_eq!(bad["error"]["code"], METHOD_NOT_FOUND);
    assert_eq!(bad["error"]["message"], "nope");
    assert!(bad.get("result").is_none());
}

#[test]
fn parse_reply_reads_result_and_error() {
    let ok = parse_reply(&json!({ "jsonrpc": "2.0", "id": 5, "result": { "x": 1 } })).unwrap();
    assert!(!ok.is_error());
    assert_eq!(ok.id, json!(5));

    let bad =
        parse_reply(&json!({ "jsonrpc": "2.0", "id": 5, "error": { "code": -1, "message": "e" } }))
            .unwrap();
    assert!(bad.is_error());

    // Neither result nor error is not a reply.
    assert!(parse_reply(&json!({ "jsonrpc": "2.0", "id": 5 })).is_none());
}

#[test]
fn envelopes_carry_jsonrpc_version() {
    assert_eq!(request_envelope(1, "m", json!({}))["jsonrpc"], "2.0");
    let note = notification_envelope("n", json!({}));
    assert_eq!(note["jsonrpc"], "2.0");
    // A notification carries no id.
    assert!(note.get("id").is_none());
}
