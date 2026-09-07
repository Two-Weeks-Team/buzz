//! Real loopback HTTP contract tests, including the public CLI dispatch seam.
use super::BuzzClient;
use crate::error::{exit_code, is_retryable_error};
use axum::{http::HeaderMap, routing::post, Router};
use nostr::{EventBuilder, Keys, Kind};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Peer {
    url: String,
    requests: Arc<AtomicUsize>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn peer(body: &str) -> Peer {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let body = body.to_owned();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = requests.clone();
    let app = Router::new().route(
        "/query",
        post(move |headers: HeaderMap| {
            let body = body.clone();
            let count = count.clone();
            async move {
                assert!(headers["authorization"]
                    .to_str()
                    .unwrap()
                    .starts_with("Nostr "));
                count.fetch_add(1, Ordering::SeqCst);
                body
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Peer {
        url,
        requests,
        task,
    }
}

#[tokio::test]
async fn query_response_rejects_invalid_shapes_without_retry_or_body_disclosure() {
    for body in [
        "",
        "not-json-SENSITIVE",
        "[",
        "null",
        "42",
        "\"SENSITIVE\"",
        r#"{"error":"SENSITIVE"}"#,
        "[null]",
        "[42]",
        "[[]]",
        "[{}, false]",
    ] {
        let peer = peer(body).await;
        let client = BuzzClient::new(peer.url.clone(), Keys::generate(), None, None).unwrap();
        let error = client
            .query_multi(&[serde_json::json!({"kinds":[9]})])
            .await
            .unwrap_err();
        assert_eq!(exit_code(&error), 4, "{body}");
        assert!(!is_retryable_error(&error));
        assert_eq!(
            error.to_string(),
            "invalid query response: expected JSON array of event objects"
        );
        assert_eq!(peer.requests.load(Ordering::SeqCst), 1, "{body}");
    }
}

#[tokio::test]
async fn query_response_preserves_empty_and_signed_extended_events_verbatim() {
    let event = EventBuilder::new(Kind::TextNote, "actual signed fixture")
        .sign_with_keys(&Keys::generate())
        .unwrap();
    let mut value = serde_json::to_value(event).unwrap();
    value["relay_extra"] = serde_json::json!({"score":0.25});
    for body in [
        "  []\n".to_owned(),
        serde_json::to_string(&vec![value]).unwrap(),
    ] {
        let peer = peer(&body).await;
        let client = BuzzClient::new(peer.url.clone(), Keys::generate(), None, None).unwrap();
        assert_eq!(
            client
                .query(&serde_json::json!({"kinds":[9]}))
                .await
                .unwrap(),
            body
        );
        assert_eq!(peer.requests.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn query_response_cli_commands_fail_instead_of_printing_empty_success() {
    // These callers historically used fallbacks after parsing the query response.
    let commands = [
        vec![
            "messages",
            "get",
            "--channel",
            "00000000-0000-0000-0000-000000000001",
        ],
        vec!["messages", "search", "--query", "fixture"],
        vec!["channels", "list"],
        vec![
            "users",
            "presence",
            "--pubkeys",
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        ],
        vec!["feed", "get"],
    ];
    for body in [
        "not-json-SENSITIVE",
        "null",
        r#"{"error":"SENSITIVE"}"#,
        "[null]",
        "[]",
    ] {
        for command in &commands {
            let peer = peer(body).await;
            let secret = format!("{:064x}", 1);
            let mut args = vec!["buzz", "--relay", &peer.url, "--private-key", &secret];
            args.extend(command.iter().copied());
            let code = crate::run_from_args(args).await;
            assert_eq!(
                code,
                if body == "[]" { 0 } else { 4 },
                "{command:?}: {body}"
            );
            assert!(peer.requests.load(Ordering::SeqCst) > 0);
        }
    }
}
