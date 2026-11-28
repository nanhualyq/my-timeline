//! The WebDAV side of syncing: where the document lives, how it is read and
//! written, and how a failure is named.
//!
//! Deliberately small. WebDAV is a file store: there is no merge here, no
//! transaction spanning two files, and no locking. The only concurrency
//! primitive it offers is a conditional request, so every decision about
//! *what* to write lives in `sync.rs` and this module only carries bytes and
//! reports what the server said.

use crate::db::SyncConfig;
use std::time::Duration;

/// Used when the reader leaves the remote path empty. A document name rather
/// than the collection root, so an empty field cannot mean "overwrite the
/// collection".
pub const DEFAULT_DOCUMENT_NAME: &str = "my-timeline-sync.json";

pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

const USER_AGENT: &str = "my-timeline-sync/0.1";

/// A resolved destination: the full document URL plus the credentials and the
/// certificate decision for this connection.
#[derive(Clone)]
pub struct Target {
    pub document_url: String,
    pub username: String,
    pub password: String,
    pub accept_invalid_certs: bool,
}

impl Target {
    pub fn from_config(config: &SyncConfig) -> Result<Target, String> {
        Ok(Target {
            document_url: document_url(&config.url, &config.path)?,
            username: config.username.clone(),
            password: config.password.clone(),
            accept_invalid_certs: config.accept_invalid_certs,
        })
    }
}

/// Resolve the document URL from the server URL and the configured path.
///
/// The server URL names a collection, so it is treated as a directory: a path
/// is always resolved inside it, whether or not the reader typed a leading
/// slash, and an empty path means the default document name.
pub fn document_url(server_url: &str, path: &str) -> Result<String, String> {
    let server_url = server_url.trim();
    if server_url.is_empty() {
        return Err("no server URL is configured".into());
    }
    let mut base = url::Url::parse(server_url)
        .map_err(|e| format!("the server URL is not a valid URL: {e}"))?;
    if base.scheme() != "http" && base.scheme() != "https" {
        return Err(format!(
            "the server URL must start with http:// or https://, not {}://",
            base.scheme()
        ));
    }
    if !base.path().ends_with('/') {
        let with_slash = format!("{}/", base.path());
        base.set_path(&with_slash);
    }
    let path = path.trim().trim_start_matches('/');
    let path = if path.is_empty() {
        DEFAULT_DOCUMENT_NAME
    } else {
        path
    };
    base.join(path)
        .map(|url| url.to_string())
        .map_err(|e| format!("cannot build the document URL: {e}"))
}

/// A client built for one connection. Built per call rather than shared, so the
/// certificate relaxation can never apply to anything but this connection's
/// requests — in particular, never to feed fetching, which builds its own
/// client.
fn client(target: &Target) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(USER_AGENT);
    if target.accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }
    builder.build().map_err(|e| format!("cannot set up the HTTP client: {e}"))
}

fn request(
    target: &Target,
    method: reqwest::Method,
    body: Option<String>,
) -> Result<reqwest::RequestBuilder, String> {
    let client = client(target)?;
    let mut builder = client.request(method, &target.document_url);
    if !target.username.is_empty() {
        builder = builder.basic_auth(&target.username, Some(&target.password));
    }
    if let Some(body) = body {
        builder = builder.body(body);
    }
    Ok(builder)
}

/// What a read produced.
pub enum GetOutcome {
    /// The remote document is the version already read; nothing was downloaded.
    NotModified,
    Fetched { text: String, etag: Option<String> },
    /// There is no document there yet. Distinct from a failure: the sync creates
    /// it rather than reporting an error.
    Missing,
}

/// What a write produced.
pub enum PutOutcome {
    Written { etag: Option<String> },
    /// The document changed since it was read. `sync.rs` re-reads and re-merges
    /// rather than overwriting.
    PreconditionFailed,
}

/// Read the document, telling the server which version is already known.
pub async fn get_document(
    target: &Target,
    if_none_match: Option<&str>,
) -> Result<GetOutcome, String> {
    let mut builder = request(target, reqwest::Method::GET, None)?;
    if let Some(etag) = if_none_match {
        builder = builder.header(reqwest::header::IF_NONE_MATCH, etag);
    }
    let response = builder.send().await.map_err(|e| describe_transport(&e))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(GetOutcome::NotModified);
    }
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(GetOutcome::Missing);
    }
    if !status.is_success() {
        return Err(describe_status(status.as_u16()));
    }
    let etag = header(&response, reqwest::header::ETAG);
    let text = response
        .text()
        .await
        .map_err(|e| describe_transport(&e))?;
    Ok(GetOutcome::Fetched { text, etag })
}

/// Write the document, conditional on the version that was read.
pub async fn put_document(
    target: &Target,
    text: &str,
    if_match: Option<&str>,
) -> Result<PutOutcome, String> {
    let mut builder = request(target, reqwest::Method::PUT, Some(text.to_string()))?;
    if let Some(etag) = if_match {
        builder = builder.header(reqwest::header::IF_MATCH, etag);
    }
    let response = builder.send().await.map_err(|e| describe_transport(&e))?;
    let status = response.status();
    if status == reqwest::StatusCode::PRECONDITION_FAILED {
        return Ok(PutOutcome::PreconditionFailed);
    }
    if !status.is_success() {
        return Err(describe_status(status.as_u16()));
    }
    Ok(PutOutcome::Written {
        etag: header(&response, reqwest::header::ETAG),
    })
}

/// The outcome of testing a connection, so the reader learns what is wrong
/// before the next sync finds out for them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConnectionTest {
    pub document_exists: bool,
    pub document_url: String,
}

/// Check that the server answers `GET` for the configured document. Reaching a
/// 404 counts as a working connection: the collection is there, the document is
/// simply not created yet.
pub async fn test_connection(target: &Target) -> Result<ConnectionTest, String> {
    let response = request(target, reqwest::Method::GET, None)?
        .send()
        .await
        .map_err(|e| describe_transport(&e))?;
    let status = response.status();
    match status.as_u16() {
        200..=299 | 304 => Ok(ConnectionTest {
            document_exists: true,
            document_url: target.document_url.clone(),
        }),
        // Reachable and authenticated, but the document is not there yet.
        404 => Ok(ConnectionTest {
            document_exists: false,
            document_url: target.document_url.clone(),
        }),
        other => Err(describe_status(other)),
    }
}

fn header(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Why a status code is a problem, in words rather than in digits.
pub fn describe_status(status: u16) -> String {
    match status {
        401 | 403 => format!("the server rejected the username or password (HTTP {status})"),
        404 => "the document was not found: check the server URL and the remote path (HTTP 404)"
            .to_string(),
        405 => format!(
            "the server refused this method (HTTP {status}); the path may not be a writable WebDAV collection"
        ),
        409 => format!(
            "the collection is missing or not writable (HTTP {status}); a WebDAV server needs the collection to exist"
        ),
        412 => "the document changed on the server (HTTP 412)".to_string(),
        code if code >= 500 => format!("the server failed to handle the request (HTTP {code})"),
        code => format!("unexpected answer from the server (HTTP {code})"),
    }
}

/// Every level of the error, outermost last, so the cause that actually
/// identifies the failure is in the message. `reqwest`'s own `Display` says the
/// same sentence for a DNS failure, a refused connection, and a certificate
/// rejection.
fn flatten(error: &dyn std::error::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !parts.iter().any(|p| p == &text) {
            parts.push(text);
        }
        source = cause.source();
    }
    parts.join(": ")
}

fn looks_like_certificate(chain: &str) -> bool {
    let chain = chain.to_ascii_lowercase();
    chain.contains("certificate")
        || chain.contains("unknownissuer")
        || chain.contains("invalid peer")
        || chain.contains("self-signed")
}

/// Why a request did not get an answer, in words rather than in a stack of
/// causes.
pub fn describe_transport(error: &reqwest::Error) -> String {
    let chain = flatten(error);
    if looks_like_certificate(&chain) {
        // Worth separating: the fix is the "accept invalid certificates" switch
        // or a proper certificate, not a network investigation.
        format!("the server's certificate was rejected: {chain}")
    } else if error.is_timeout() {
        format!("the server did not answer in time: {chain}")
    } else if error.is_connect() {
        format!("cannot reach the server: {chain}")
    } else {
        format!("the request failed: {chain}")
    }
}

#[cfg(test)]
pub mod stub {
    //! A local WebDAV-ish server, enough for the sync tests: it holds one
    //! document and one version tag, honours `If-None-Match` and `If-Match`,
    //! records every request, and can play the part of another installation
    //! writing between a read and a write.

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Default)]
    pub struct Recorded {
        pub method: String,
        pub path: String,
        pub if_none_match: Option<String>,
        pub if_match: Option<String>,
        pub authorization: Option<String>,
        pub body: String,
    }

    #[derive(Debug, Default)]
    struct State {
        body: Option<String>,
        version: u64,
        requests: Vec<Recorded>,
        interference: Option<String>,
        forced_status: Option<u16>,
        always_conflict: bool,
    }

    impl State {
        fn etag(&self) -> String {
            format!("\"{}\"", self.version)
        }
    }

    pub struct Stub {
        pub base_url: String,
        state: Arc<Mutex<State>>,
    }

    impl Stub {
        pub fn start() -> Stub {
            Self::spawn(false)
        }

        /// A server presenting a self-signed certificate for `127.0.0.1`.
        pub fn start_tls() -> Stub {
            Self::spawn(true)
        }

        fn spawn(tls: bool) -> Stub {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let state = Arc::new(Mutex::new(State::default()));
            let shared = Arc::clone(&state);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else { continue };
                    let shared = Arc::clone(&shared);
                    std::thread::spawn(move || {
                        if tls {
                            match tls_config() {
                                Ok(config) => match rustls::ServerConnection::new(config) {
                                    Ok(conn) => {
                                        let mut stream = rustls::StreamOwned::new(conn, stream);
                                        handle(&mut stream, &shared);
                                    }
                                    Err(_) => {}
                                },
                                Err(_) => {}
                            }
                        } else {
                            let mut stream: TcpStream = stream;
                            handle(&mut stream, &shared);
                        }
                    });
                }
            });
            Stub {
                base_url: format!("{}://{addr}/dav", if tls { "https" } else { "http" }),
                state,
            }
        }

        pub fn document_url(&self) -> String {
            format!("{}/state.json", self.base_url)
        }

        /// Replace the stored document as if written by someone else.
        pub fn set_body(&self, body: &str) {
            let mut state = self.state.lock().unwrap();
            state.body = Some(body.to_string());
            state.version += 1;
        }

        /// Another installation writes just before the next `PUT` is checked.
        pub fn interfere_on_next_put(&self, body: &str) {
            self.state.lock().unwrap().interference = Some(body.to_string());
        }

        /// Reject every write, as two installations fighting would.
        pub fn always_conflict(&self) {
            self.state.lock().unwrap().always_conflict = true;
        }

        pub fn force_status(&self, status: u16) {
            self.state.lock().unwrap().forced_status = Some(status);
        }

        pub fn body(&self) -> Option<String> {
            self.state.lock().unwrap().body.clone()
        }

        pub fn requests(&self) -> Vec<Recorded> {
            self.state.lock().unwrap().requests.clone()
        }

        pub fn put_count(&self) -> usize {
            self.requests().iter().filter(|r| r.method == "PUT").count()
        }
    }

    fn handle<S: Read + Write>(stream: &mut S, state: &Arc<Mutex<State>>) {
        let Some(request) = read_request(stream) else { return };
        let (status, etag, body) = {
            let mut state = state.lock().unwrap();
            state.requests.push(request.clone());
            respond(&mut state, &request)
        };
        let mut head = format!("HTTP/1.1 {status} {}\r\n", reason(status));
        if let Some(etag) = etag {
            head.push_str(&format!("ETag: {etag}\r\n"));
        }
        if let Some(body) = &body {
            head.push_str(&format!("Content-Length: {}\r\n", body.len()));
        } else {
            head.push_str("Content-Length: 0\r\n");
        }
        head.push_str("Connection: close\r\n\r\n");
        let _ = stream.write_all(head.as_bytes());
        if let Some(body) = body {
            let _ = stream.write_all(body.as_bytes());
        }
        let _ = stream.flush();
    }

    /// Returns (status, etag, body).
    fn respond(state: &mut State, request: &Recorded) -> (u16, Option<String>, Option<String>) {
        if let Some(forced) = state.forced_status.take() {
            return (forced, None, Some("forced".to_string()));
        }
        match request.method.as_str() {
            "GET" | "HEAD" => {
                let Some(body) = state.body.clone() else {
                    return (404, None, Some("missing".to_string()));
                };
                let etag = state.etag();
                if request.if_none_match.as_deref() == Some(etag.as_str()) {
                    return (304, Some(etag), None);
                }
                (200, Some(etag), Some(body))
            }
            "PUT" => {
                if state.always_conflict {
                    state.version += 1;
                    return (412, Some(state.etag()), Some("conflict".to_string()));
                }
                if let Some(interference) = state.interference.take() {
                    state.body = Some(interference);
                    state.version += 1;
                }
                if let Some(expected) = request.if_match.as_deref() {
                    if state.body.is_none() || expected != state.etag() {
                        return (412, Some(state.etag()), Some("stale".to_string()));
                    }
                }
                state.body = Some(request.body.clone());
                state.version += 1;
                let etag = state.etag();
                (200, Some(etag), None)
            }
            _ => (405, None, None),
        }
    }

    fn reason(status: u16) -> &'static str {
        match status {
            200 => "OK",
            304 => "Not Modified",
            401 => "Unauthorized",
            404 => "Not Found",
            405 => "Method Not Allowed",
            409 => "Conflict",
            412 => "Precondition Failed",
            _ => "Unknown",
        }
    }

    fn read_request<S: Read + Write>(stream: &mut S) -> Option<Recorded> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 2048];
        loop {
            let n = stream.read(&mut chunk).ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&chunk[..n]);
            let Some(split) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let head = String::from_utf8_lossy(&buf[..split]).to_string();
            let mut body = buf[split + 4..].to_vec();
            let length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            while body.len() < length {
                let n = stream.read(&mut chunk).ok()?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..n]);
            }
            return Some(parse(&head, &body));
        }
    }

    fn parse(head: &str, body: &[u8]) -> Recorded {
        let mut lines = head.lines();
        let mut request_line = lines.next().unwrap_or_default().split_whitespace();
        let method = request_line.next().unwrap_or_default().to_string();
        let path = request_line.next().unwrap_or_default().to_string();
        let mut recorded = Recorded {
            method,
            path,
            body: String::from_utf8_lossy(body).to_string(),
            ..Default::default()
        };
        for line in lines {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim().to_string();
            if name.eq_ignore_ascii_case("if-none-match") {
                recorded.if_none_match = Some(value);
            } else if name.eq_ignore_ascii_case("if-match") {
                recorded.if_match = Some(value);
            } else if name.eq_ignore_ascii_case("authorization") {
                recorded.authorization = Some(value);
            }
        }
        recorded
    }

    /// The committed self-signed certificate, so the client can be shown a
    /// certificate it does not trust without reaching the network.
    const CERT_PEM: &str = include_str!("../testdata/localhost-cert.pem");
    const KEY_PEM: &str = include_str!("../testdata/localhost-key.pem");

    fn pem_bytes(pem: &str, label: &str) -> Vec<u8> {
        use base64::Engine;
        let begin = format!("-----BEGIN {label}-----");
        let end = format!("-----END {label}-----");
        let body = pem
            .split(&begin)
            .nth(1)
            .expect("pem begins")
            .split(&end)
            .next()
            .expect("pem ends");
        let cleaned: String = body.chars().filter(|c| !c.is_whitespace()).collect();
        base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .expect("pem body is base64")
    }

    fn tls_config() -> Result<Arc<rustls::ServerConfig>, Box<dyn std::error::Error>> {
        use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
        let cert = CertificateDer::from(pem_bytes(CERT_PEM, "CERTIFICATE"));
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(pem_bytes(KEY_PEM, "PRIVATE KEY")));
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)?;
        Ok(Arc::new(config))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stub::Stub;

    fn target(url: &str) -> Target {
        Target {
            document_url: url.to_string(),
            username: String::new(),
            password: String::new(),
            accept_invalid_certs: false,
        }
    }

    fn credentialled(url: &str, username: &str, password: &str) -> Target {
        Target {
            username: username.to_string(),
            password: password.to_string(),
            ..target(url)
        }
    }

    // --- Where the document lives ---------------------------------------

    #[test]
    fn an_empty_path_resolves_to_the_default_document_name() {
        assert_eq!(
            document_url("https://dav.example/dav", "").unwrap(),
            "https://dav.example/dav/my-timeline-sync.json"
        );
        assert_eq!(
            document_url("https://dav.example/dav", "  ").unwrap(),
            "https://dav.example/dav/my-timeline-sync.json"
        );
    }

    #[test]
    fn the_server_url_is_a_collection_so_the_path_resolves_inside_it() {
        for path in ["state.json", "/state.json", " /state.json "] {
            assert_eq!(
                document_url("https://dav.example/dav", path).unwrap(),
                "https://dav.example/dav/state.json",
                "path {path:?} must resolve inside the collection"
            );
        }
        assert_eq!(
            document_url("https://dav.example/dav/", "nested/state.json").unwrap(),
            "https://dav.example/dav/nested/state.json"
        );
    }

    #[test]
    fn a_server_url_that_is_not_http_is_rejected() {
        let error = document_url("ftp://dav.example/dav", "state.json").unwrap_err();
        assert!(error.contains("http"), "{error}");
        let error = document_url("dav.example/dav", "state.json").unwrap_err();
        assert!(error.contains("valid URL"), "{error}");
        let error = document_url("   ", "state.json").unwrap_err();
        assert!(error.contains("no server URL"), "{error}");
    }

    // --- Reading and writing --------------------------------------------

    #[tokio::test]
    async fn a_read_reports_the_version_tag_and_a_second_read_can_skip_the_body() {
        let stub = Stub::start();
        stub.set_body("{\"doc_version\":1}");
        let target = target(&stub.document_url());

        let first = get_document(&target, None).await.unwrap();
        let etag = match first {
            GetOutcome::Fetched { text, etag } => {
                assert_eq!(text, "{\"doc_version\":1}");
                etag.expect("the server sets an ETag")
            }
            GetOutcome::NotModified | GetOutcome::Missing => panic!("first read must fetch"),
        };

        let second = get_document(&target, Some(&etag)).await.unwrap();
        assert!(matches!(second, GetOutcome::NotModified));

        let requests = stub.requests();
        assert_eq!(requests[0].if_none_match, None);
        assert_eq!(requests[1].if_none_match.as_deref(), Some(etag.as_str()));
    }

    #[tokio::test]
    async fn a_write_carries_the_version_it_replaces() {
        let stub = Stub::start();
        stub.set_body("old");
        let target = target(&stub.document_url());
        let etag = match get_document(&target, None).await.unwrap() {
            GetOutcome::Fetched { etag, .. } => etag.unwrap(),
            _ => panic!("expected a fetch"),
        };

        let outcome = put_document(&target, "new", Some(&etag)).await.unwrap();

        assert!(matches!(outcome, PutOutcome::Written { .. }));
        assert_eq!(stub.body().as_deref(), Some("new"));
        assert_eq!(
            stub.requests().last().unwrap().if_match.as_deref(),
            Some(etag.as_str())
        );
    }

    #[tokio::test]
    async fn a_stale_write_is_reported_as_a_precondition_failure_not_an_error() {
        let stub = Stub::start();
        stub.set_body("old");
        let target = target(&stub.document_url());
        let etag = match get_document(&target, None).await.unwrap() {
            GetOutcome::Fetched { etag, .. } => etag.unwrap(),
            _ => panic!("expected a fetch"),
        };
        stub.set_body("someone else");

        let outcome = put_document(&target, "mine", Some(&etag)).await.unwrap();

        assert!(matches!(outcome, PutOutcome::PreconditionFailed));
        assert_eq!(stub.body().as_deref(), Some("someone else"));
    }

    #[tokio::test]
    async fn a_missing_document_is_reported_as_missing_not_as_a_failure() {
        let stub = Stub::start();
        let target = target(&stub.document_url());
        let outcome = get_document(&target, None).await.unwrap();
        assert!(
            matches!(outcome, GetOutcome::Missing),
            "the sync creates a missing document instead of failing"
        );
    }

    // --- Naming failures -------------------------------------------------

    #[test]
    fn a_rejected_credential_and_a_rejected_certificate_read_differently() {
        let credential = describe_status(401);
        assert!(credential.contains("username or password"), "{credential}");

        let certificate = describe_transport_text(
            "error sending request for url (https://dav.example/state.json): \
             invalid peer certificate: UnknownIssuer",
        )
        .expect("a certificate chain is a certificate failure");
        assert!(certificate.contains("certificate"), "{certificate}");
        assert_ne!(credential, certificate);
    }

    #[test]
    fn a_missing_collection_and_a_server_failure_read_differently() {
        assert!(describe_status(404).contains("remote path"));
        assert!(describe_status(409).contains("collection"));
        assert!(describe_status(405).contains("WebDAV"));
        assert!(describe_status(503).contains("server failed"));
        assert!(describe_status(418).contains("unexpected answer"));
    }

    /// The transport classifier, without needing a `reqwest::Error` in hand.
    fn describe_transport_text(chain: &str) -> Option<String> {
        if looks_like_certificate(chain) {
            Some(format!("the server's certificate was rejected: {chain}"))
        } else {
            None
        }
    }

    // --- Testing a connection -------------------------------------------

    #[tokio::test]
    async fn testing_a_connection_reports_whether_the_document_exists() {
        let stub = Stub::start();
        let target = target(&stub.document_url());
        let test = test_connection(&target).await.unwrap();
        assert!(!test.document_exists, "the collection is there, the document is not");

        stub.set_body("{}");
        let test = test_connection(&target).await.unwrap();
        assert!(test.document_exists);
    }

    #[tokio::test]
    async fn testing_a_connection_names_rejected_credentials() {
        let stub = Stub::start();
        stub.force_status(401);
        let target = credentialled(&stub.document_url(), "me", "secret");

        let error = test_connection(&target).await.unwrap_err();

        assert!(error.contains("username or password"), "{error}");
    }

    #[tokio::test]
    async fn credentials_are_sent_when_a_username_is_configured() {
        let stub = Stub::start();
        stub.set_body("{}");
        let target = credentialled(&stub.document_url(), "me", "secret");

        get_document(&target, None).await.unwrap();

        let authorization = stub.requests()[0].authorization.clone().unwrap();
        assert!(authorization.starts_with("Basic "), "{authorization}");
        let decoded = {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(authorization.trim_start_matches("Basic "))
                .unwrap()
        };
        assert_eq!(String::from_utf8(decoded).unwrap(), "me:secret");
    }

    #[tokio::test]
    async fn no_credentials_are_sent_when_the_username_is_empty() {
        let stub = Stub::start();
        stub.set_body("{}");
        get_document(&target(&stub.document_url()), None).await.unwrap();
        assert_eq!(stub.requests()[0].authorization, None);
    }

    // --- Certificates ----------------------------------------------------

    #[tokio::test]
    async fn a_self_signed_certificate_is_rejected_unless_it_is_accepted() {
        let stub = Stub::start_tls();
        stub.set_body("{}");
        let url = stub.document_url();

        let strict = target(&url);
        let error = get_document(&strict, None).await.err().expect("untrusted certificate");
        assert!(
            error.contains("certificate"),
            "the message must name the certificate: {error}"
        );

        let relaxed = Target {
            accept_invalid_certs: true,
            ..target(&url)
        };
        match get_document(&relaxed, None).await.unwrap() {
            GetOutcome::Fetched { text, .. } => assert_eq!(text, "{}"),
            _ => panic!("expected a fetch"),
        }
    }

    #[test]
    fn accepting_invalid_certificates_comes_only_from_the_configuration() {
        let mut config = SyncConfig {
            url: "https://dav.example/dav".into(),
            ..Default::default()
        };
        assert!(!Target::from_config(&config).unwrap().accept_invalid_certs);

        config.accept_invalid_certs = true;
        assert!(
            Target::from_config(&config).unwrap().accept_invalid_certs,
            "the switch reaches the sync client"
        );
        // The feed fetcher never consults this: `fetch.rs` builds its own
        // client, so no relaxation can leak into it.
    }
}
