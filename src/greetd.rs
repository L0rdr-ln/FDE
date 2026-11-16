//! The login manager side of logging in: greetd's protocol (a socket whose path is in
//! `$GREETD_SOCK`; every message is a native-endian u32 length and then JSON). The greeter asks
//! greetd to create a session for a user name, answers its questions (the password), and tells it
//! what to start when that worked.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    CreateSession { username: String },
    PostAuthMessageResponse { response: Option<String> },
    StartSession { cmd: Vec<String>, env: Vec<String> },
    CancelSession,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    AuthError,
    Error,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMessageType {
    Visible,
    Secret,
    Info,
    Error,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Success,
    Error { error_type: ErrorType, description: String },
    AuthMessage { auth_message_type: AuthMessageType, auth_message: String },
}

pub fn send<W: Write>(w: &mut W, request: &Request) -> std::io::Result<()> {
    let body = serde_json::to_vec(request).map_err(std::io::Error::other)?;
    w.write_all(&(body.len() as u32).to_ne_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

pub fn receive<R: Read>(r: &mut R) -> std::io::Result<Response> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_ne_bytes(len) as usize;
    if len > 1 << 20 {
        return Err(std::io::Error::other("greetd sent a message that is far too long"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    serde_json::from_slice(&body).map_err(std::io::Error::other)
}

/// Where the login stands after a message from greetd.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// greetd asks for something (`secret`: it is a password, show dots); `notes` are the messages
    /// it sent before ("Password expires soon")
    Ask { secret: bool, prompt: String, notes: Vec<String> },
    /// the user is who they say: the session may be started
    Authenticated,
    /// it did not work; `wrong_secret` is the "wrong password" kind, the rest is a real error
    Failed { wrong_secret: bool, message: String },
}

pub struct Client<S: Read + Write> {
    stream: S,
}

impl Client<UnixStream> {
    /// Connect to the greetd that started this greeter (`$GREETD_SOCK`).
    pub fn connect_from_env() -> std::io::Result<Client<UnixStream>> {
        let path = std::env::var_os("GREETD_SOCK").ok_or_else(|| std::io::Error::other("GREETD_SOCK is not set: this program is started by greetd"))?;
        Ok(Client { stream: UnixStream::connect(path)? })
    }

    pub fn connect(path: &std::path::Path) -> std::io::Result<Client<UnixStream>> {
        Ok(Client { stream: UnixStream::connect(path)? })
    }
}

impl<S: Read + Write> Client<S> {
    pub fn new(stream: S) -> Client<S> {
        Client { stream }
    }

    fn call(&mut self, request: &Request) -> std::io::Result<Response> {
        send(&mut self.stream, request)?;
        receive(&mut self.stream)
    }

    /// Follow greetd's answers until it wants something from the user or is done. Messages that
    /// need no answer (info, error text) are collected.
    fn settle(&mut self, mut response: Response) -> std::io::Result<Step> {
        let mut notes = Vec::new();
        // a greetd that never stops sending messages does not hold the greeter for ever
        for _ in 0..64 {
            match response {
                Response::Success => return Ok(Step::Authenticated),
                Response::Error { error_type, description } => {
                    return Ok(Step::Failed { wrong_secret: error_type == ErrorType::AuthError, message: description });
                }
                Response::AuthMessage { auth_message_type: AuthMessageType::Visible, auth_message } => {
                    return Ok(Step::Ask { secret: false, prompt: auth_message, notes });
                }
                Response::AuthMessage { auth_message_type: AuthMessageType::Secret, auth_message } => {
                    return Ok(Step::Ask { secret: true, prompt: auth_message, notes });
                }
                Response::AuthMessage { auth_message, .. } => {
                    notes.push(auth_message);
                    response = self.call(&Request::PostAuthMessageResponse { response: None })?;
                }
            }
        }
        Err(std::io::Error::other("greetd sent too many messages in a row"))
    }

    /// Ask greetd to start logging `username` in.
    pub fn begin(&mut self, username: &str) -> std::io::Result<Step> {
        let r = self.call(&Request::CreateSession { username: username.to_string() })?;
        self.settle(r)
    }

    /// Answer what greetd asked.
    pub fn answer(&mut self, text: &str) -> std::io::Result<Step> {
        let r = self.call(&Request::PostAuthMessageResponse { response: Some(text.to_string()) })?;
        self.settle(r)
    }

    /// Give up the login in progress (after a wrong password, before trying again).
    pub fn cancel(&mut self) -> std::io::Result<()> {
        self.call(&Request::CancelSession).map(|_| ())
    }

    /// The user is authenticated: start their session. Err with greetd's words if it cannot.
    pub fn start(&mut self, cmd: Vec<String>, env: Vec<String>) -> std::io::Result<Result<(), String>> {
        Ok(match self.call(&Request::StartSession { cmd, env })? {
            Response::Success => Ok(()),
            Response::Error { description, .. } => Err(description),
            Response::AuthMessage { .. } => Err("greetd asked for more after the login".to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// A stand-in greetd: expects these requests in this order and answers with the responses.
    fn fake_greetd(dir: &std::path::Path, script: Vec<(Request, Response)>) -> (std::path::PathBuf, std::thread::JoinHandle<Vec<Request>>) {
        let path = dir.join("greetd.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut seen = Vec::new();
            for (expected, answer) in script {
                // the same framing in the other direction: read a request as JSON
                let mut len = [0u8; 4];
                stream.read_exact(&mut len).unwrap();
                let mut body = vec![0u8; u32::from_ne_bytes(len) as usize];
                stream.read_exact(&mut body).unwrap();
                let got: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(got, serde_json::to_value(&expected).unwrap(), "greetd got the wrong request");
                seen.push(expected);
                let reply = match answer {
                    Response::Success => serde_json::json!({"type": "success"}),
                    Response::Error { error_type, description } => serde_json::json!({"type": "error", "error_type": if error_type == ErrorType::AuthError { "auth_error" } else { "error" }, "description": description}),
                    Response::AuthMessage { auth_message_type, auth_message } => {
                        let t = match auth_message_type {
                            AuthMessageType::Visible => "visible",
                            AuthMessageType::Secret => "secret",
                            AuthMessageType::Info => "info",
                            AuthMessageType::Error => "error",
                        };
                        serde_json::json!({"type": "auth_message", "auth_message_type": t, "auth_message": auth_message})
                    }
                };
                let body = serde_json::to_vec(&reply).unwrap();
                stream.write_all(&(body.len() as u32).to_ne_bytes()).unwrap();
                stream.write_all(&body).unwrap();
            }
            seen
        });
        (path, handle)
    }

    fn secret(prompt: &str) -> Response {
        Response::AuthMessage { auth_message_type: AuthMessageType::Secret, auth_message: prompt.into() }
    }

    #[test]
    fn a_login_asks_for_the_password_and_starts_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let cmd = vec!["start-fde".to_string()];
        let (path, server) = fake_greetd(
            dir.path(),
            vec![
                (Request::CreateSession { username: "alex".into() }, secret("Password: ")),
                (Request::PostAuthMessageResponse { response: Some("hunter2".into()) }, Response::Success),
                (Request::StartSession { cmd: cmd.clone(), env: vec![] }, Response::Success),
            ],
        );
        let mut c = Client::connect(&path).unwrap();
        assert_eq!(c.begin("alex").unwrap(), Step::Ask { secret: true, prompt: "Password: ".into(), notes: vec![] });
        assert_eq!(c.answer("hunter2").unwrap(), Step::Authenticated);
        assert_eq!(c.start(cmd, vec![]).unwrap(), Ok(()));
        assert_eq!(server.join().unwrap().len(), 3);
    }

    #[test]
    fn a_wrong_password_is_told_apart_and_the_login_can_start_over() {
        let dir = tempfile::tempdir().unwrap();
        let (path, server) = fake_greetd(
            dir.path(),
            vec![
                (Request::CreateSession { username: "alex".into() }, secret("Password: ")),
                (Request::PostAuthMessageResponse { response: Some("nope".into()) }, Response::Error { error_type: ErrorType::AuthError, description: "Authentication failed".into() }),
                (Request::CancelSession, Response::Success),
                (Request::CreateSession { username: "alex".into() }, secret("Password: ")),
            ],
        );
        let mut c = Client::connect(&path).unwrap();
        c.begin("alex").unwrap();
        assert_eq!(c.answer("nope").unwrap(), Step::Failed { wrong_secret: true, message: "Authentication failed".into() });
        c.cancel().unwrap();
        assert!(matches!(c.begin("alex").unwrap(), Step::Ask { secret: true, .. }), "asks again");
        server.join().unwrap();
    }

    #[test]
    fn info_messages_are_collected_and_an_unknown_user_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let info = Response::AuthMessage { auth_message_type: AuthMessageType::Info, auth_message: "Your password expires in 3 days".into() };
        let (path, server) = fake_greetd(
            dir.path(),
            vec![
                (Request::CreateSession { username: "alex".into() }, info),
                (Request::PostAuthMessageResponse { response: None }, secret("Password: ")),
                (Request::CancelSession, Response::Success),
                (Request::CreateSession { username: "nobody".into() }, Response::Error { error_type: ErrorType::Error, description: "no such user".into() }),
            ],
        );
        let mut c = Client::connect(&path).unwrap();
        assert_eq!(c.begin("alex").unwrap(), Step::Ask { secret: true, prompt: "Password: ".into(), notes: vec!["Your password expires in 3 days".into()] });
        c.cancel().unwrap();
        assert_eq!(c.begin("nobody").unwrap(), Step::Failed { wrong_secret: false, message: "no such user".into() });
        server.join().unwrap();
    }

    #[test]
    fn the_framing_is_a_native_length_and_json() {
        let mut out = Vec::new();
        send(&mut out, &Request::CancelSession).unwrap();
        assert_eq!(&out[4..], br#"{"type":"cancel_session"}"#);
        assert_eq!(u32::from_ne_bytes(out[..4].try_into().unwrap()) as usize, out.len() - 4);
        let mut huge = Vec::new();
        huge.extend_from_slice(&u32::MAX.to_ne_bytes());
        assert!(receive(&mut huge.as_slice()).is_err(), "a silly length is refused, not allocated");
    }
}
