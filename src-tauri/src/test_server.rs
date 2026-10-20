//! Local-only contract transport. Tests never contact Resend or real recipients.
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
};
pub struct Reply {
    pub status: u16,
    pub body: &'static str,
    pub headers: &'static str,
}
pub fn serve(
    replies: Vec<Reply>,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    serve_with(replies, || {})
}
pub fn serve_with(
    replies: Vec<Reply>,
    mut before_response: impl FnMut() + Send + 'static,
) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let captured = Arc::new(Mutex::new(vec![]));
    let copy = captured.clone();
    let thread = std::thread::spawn(move || {
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            let mut chunk = [0_u8; 4096];
            loop {
                let n = stream.read(&mut chunk).unwrap_or(0);
                if n == 0 {
                    break;
                }
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..index]);
                    let length = headers
                        .lines()
                        .find_map(|s| {
                            s.to_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|s| s.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= index + 4 + length {
                        break;
                    }
                }
            }
            copy.lock()
                .unwrap()
                .push(String::from_utf8_lossy(&bytes).into_owned());
            before_response();
            if reply.status == 0 {
                continue;
            }
            let response=format!("HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",reply.status,reply.body.len(),reply.headers,reply.body);
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (format!("http://{address}"), captured, thread)
}
