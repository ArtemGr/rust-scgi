##rust-scgi [![Build Status](https://travis-ci.org/ArtemGr/rust-scgi.svg?branch=master)](https://travis-ci.org/ArtemGr/rust-scgi) [![](https://img.shields.io/crates/v/scgi.svg)](https://crates.io/crates/scgi) <br>

A simple SCGI connector for Rust.<br>
<a href="http://www.rust-ci.org/ArtemGr/rust-scgi/doc/scgi/">Documentation</a>

    [dependencies.scgi]
    git = "https://github.com/ArtemGr/rust-scgi"

Example:

```rust
pub fn main() {
  let mut acceptor = TcpListener::bind (("127.0.0.1", 8083)) .listen().unwrap();
  for stream in acceptor.incoming() {
    match stream {
      Err (err) => panic! ("Accept error: {}", err),
      Ok (tcp_stream) => spawn (proc() {
        let (raw_headers, mut stream) = scgi::read_headers (tcp_stream) .unwrap();
        let headers_map = scgi::str_map (&raw_headers) .unwrap();
        let uri = headers_map["REQUEST_URI"];

        println! ("SCGI request, uri: {}, headers: {}", uri, headers_map);
        stream.write (b"Status: 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\nHi\r\n") .unwrap();
      })
    }
  }
}
```

[A full example with Result error handling.](https://github.com/ArtemGr/rust-scgi/blob/master/src/example.rs)
