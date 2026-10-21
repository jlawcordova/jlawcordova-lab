#![allow(clippy::result_large_err)]
use std::io::{Cursor, Write};
use std::sync::{Arc, Mutex};

use engine::format::stringify;
use pixellab::errors::{CommandError, Error};
use pixellab::jsonrpc::*;
use serde_json::{json, Value};

fn run(line: &str) -> Option<String> {
    let mut handle = |m: &str, p: Value, _: &Value| -> Result<Value, Failure> {
        match m {
            "echo" => Ok(p),
            "nothing" => Ok(Value::Null),
            "usage" => Err(CommandError::usage("bad flag").into()),
            "cmd" => Err(CommandError::new("nope", "not_found").with_details(json!({"path": "a"})).into()),
            "boom" => Err(Error::Internal("oops".into()).into()),
            _ => Err(RpcError::new(METHOD_NOT_FOUND, "no such method").into()),
        }
    };
    handle_line(line, &mut handle, &mut |_| {}).map(|v| stringify(&v))
}

fn err_text(f: Failure) -> String {
    stringify(&to_rpc_error(&f))
}

#[test]
fn answers_a_request_and_ignores_a_notification() {
    assert_eq!(
        run(r#"{"jsonrpc":"2.0","id":1,"method":"echo","params":{"a":1}}"#).unwrap(),
        r#"{"jsonrpc":"2.0","id":1,"result":{"a":1}}"#
    );
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":"x","method":"nothing"}"#).unwrap(), r#"{"jsonrpc":"2.0","id":"x","result":null}"#);
    assert_eq!(run(r#"{"jsonrpc":"2.0","method":"echo"}"#), None);
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":null,"method":"echo"}"#), None);
    assert_eq!(run(r#"{"jsonrpc":"2.0","method":"usage"}"#), None);
}

#[test]
fn params_default_to_an_empty_object() {
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":1,"method":"echo"}"#).unwrap(), r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":1,"method":"echo","params":null}"#).unwrap(), r#"{"jsonrpc":"2.0","id":1,"result":{}}"#);
}

#[test]
fn a_line_that_is_not_json_is_a_parse_error() {
    let out: Value = serde_json::from_str(&run("{nope").unwrap()).unwrap();
    assert_eq!(out["id"], Value::Null);
    assert_eq!(out["error"]["code"], PARSE_ERROR);
    assert!(out["error"]["message"].as_str().unwrap().starts_with("parse error: "));
}

#[test]
fn an_invalid_request_echoes_its_id() {
    let text = "invalid request: expected a JSON-RPC 2.0 request object (batches are not supported)";
    let want = |id: &str| format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{{"code":-32600,"message":"{text}"}}}}"#);
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":7}"#).unwrap(), want("7"));
    assert_eq!(run(r#"{"id":"a","method":"echo"}"#).unwrap(), want(r#""a""#));
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":3,"method":4}"#).unwrap(), want("3"));
    assert_eq!(run("[1]").unwrap(), want("null"));
    assert_eq!(run("5").unwrap(), want("null"));
    assert_eq!(run("null").unwrap(), want("null"));
}

#[test]
fn responses_are_ignored() {
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#), None);
    assert_eq!(run(r#"{"jsonrpc":"2.0","id":1,"error":{"code":1,"message":"x"}}"#), None);
}

#[test]
fn blank_lines_are_skipped() {
    assert_eq!(run(""), None);
    assert_eq!(run("   \t"), None);
}

#[test]
fn a_usage_error_is_invalid_params_with_its_code_and_details() {
    assert_eq!(err_text(CommandError::usage("bad flag").into()), r#"{"code":-32602,"message":"bad flag","data":{"code":"usage"}}"#);
    assert_eq!(
        run(r#"{"jsonrpc":"2.0","id":1,"method":"usage"}"#).unwrap(),
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"bad flag","data":{"code":"usage"}}}"#
    );
}

#[test]
fn a_command_error_is_32000() {
    let e = CommandError::new("nope", "not_found").with_details(json!({"path": "a"}));
    assert_eq!(err_text(e.into()), r#"{"code":-32000,"message":"nope","data":{"code":"not_found","details":{"path":"a"}}}"#);
    assert_eq!(err_text(CommandError::new("nope", "boom").into()), r#"{"code":-32000,"message":"nope","data":{"code":"boom"}}"#);
}

#[test]
fn an_rpc_error_keeps_its_code_and_data() {
    assert_eq!(err_text(RpcError::new(METHOD_NOT_FOUND, "no such method").into()), r#"{"code":-32601,"message":"no such method"}"#);
    assert_eq!(err_text(RpcError::new(-32001, "x").with_data(json!({"a": 1})).into()), r#"{"code":-32001,"message":"x","data":{"a":1}}"#);
}

#[test]
fn an_internal_error_is_32603_and_logged() {
    assert_eq!(err_text(Error::Internal("oops".into()).into()), r#"{"code":-32603,"message":"oops","data":{"code":"internal"}}"#);
    let mut logs: Vec<String> = Vec::new();
    let mut handle = |_: &str, _: Value, _: &Value| -> Result<Value, Failure> { Err(Error::Internal("oops".into()).into()) };
    let out = handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"boom"}"#, &mut handle, &mut |t| logs.push(t.into()));
    assert_eq!(stringify(&out.unwrap()), r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32603,"message":"oops","data":{"code":"internal"}}}"#);
    assert_eq!(logs, vec!["internal error in boom: oops".to_string()]);
    // A command error is not logged.
    let mut logs: Vec<String> = Vec::new();
    let mut handle = |_: &str, _: Value, _: &Value| -> Result<Value, Failure> { Err(CommandError::usage("x").into()) };
    handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"m"}"#, &mut handle, &mut |t| logs.push(t.into()));
    assert!(logs.is_empty());
}

#[derive(Clone, Default)]
struct Sink(Arc<Mutex<Vec<u8>>>);
impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Sink {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

#[test]
fn output_stops_after_stop() {
    let sink = Sink::default();
    let out = Output::new(Box::new(sink.clone()));
    let other = out.clone();
    out.send(&json!({"a": 1}));
    other.notify("ping", json!({"n": 2}));
    assert_eq!(sink.text(), "{\"a\":1}\n{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"params\":{\"n\":2}}\n");
    other.stop();
    out.send(&json!({"b": 1}));
    out.notify("late", json!({}));
    assert_eq!(sink.text().lines().count(), 2);
}

#[test]
fn serve_lines_answers_in_order() {
    let sink = Sink::default();
    let out = Output::new(Box::new(sink.clone()));
    let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"echo\",\"params\":{\"n\":1}}\n\n{\"jsonrpc\":\"2.0\",\"method\":\"echo\"}\nnot json\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"nothing\"}";
    let mut handle = |m: &str, p: Value, _: &Value| -> Result<Value, Failure> {
        if m == "echo" {
            Ok(p)
        } else {
            Ok(Value::Null)
        }
    };
    serve_lines(Cursor::new(input), &out, &mut handle, &mut |_| {});
    let lines: Vec<String> = sink.text().lines().map(String::from).collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], r#"{"jsonrpc":"2.0","id":1,"result":{"n":1}}"#);
    assert!(lines[1].starts_with(r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error: "#));
    assert_eq!(lines[2], r#"{"jsonrpc":"2.0","id":2,"result":null}"#);
}

#[test]
fn crlf_lines_are_read() {
    let seen = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
    let s2 = seen.clone();
    let h = spawn_lines(Cursor::new("a\r\nb\n\nc"), move |l| s2.lock().unwrap().push(l));
    h.join().unwrap();
    let want: Vec<Option<String>> = vec![Some("a".into()), Some("b".into()), Some("".into()), Some("c".into()), None];
    assert_eq!(*seen.lock().unwrap(), want);
}
