//! SCGI parser.
//!
//! This is partly a port of my Java parser: https://gist.github.com/ArtemGr/38425.

#![feature(slicing_syntax)]
#![feature(default_type_params)]

extern crate rustc;

use rustc::util::nodemap::FnvHasher;  // http://www.reddit.com/r/rust/comments/2l4kxf/std_hashmap_is_slow/
use std::collections::HashMap;
use std::error::FromError;
use std::io::{BufferedStream, IoError};
use std::io::net::tcp::{TcpStream};
use std::str::from_utf8;

#[cfg(test)] use std::io::{Listener, Acceptor};
#[cfg(test)] use std::io::net::tcp::{TcpListener};
#[cfg(test)] use std::io::timer::sleep;
#[cfg(test)] use std::time::duration::Duration;

/// SCGI parsing errors.
#[deriving(Show)]
pub enum ScgiError {
  /// Length can't be UTF-8 decoded to a string or an integer.
  BadLength,
  /// Netstring sanity checks fail.
  WrongLength (String),
  /// Error parsing the zero-terminated HTTP headers.
  WrongHeaders,
  /// IoError, like when connection closed prematurely.
  IO (IoError)
}
impl FromError<IoError> for ScgiError {
  fn from_error (io_error: IoError) -> ScgiError {IO (io_error)}
}

/// Parse the headers, invoking the `header` closure for every header parsed.
///
/// Returns the `tcp_stream` wrapped into a `BufferedStream`.<br>
/// You should use it to read the rest of the query and send the response.
pub fn scgi_parse (tcp_stream: TcpStream, header: |&str,&str|) -> Result<BufferedStream<TcpStream>, ScgiError> {
  let mut stream = BufferedStream::new (tcp_stream);
  let mut headers: Vec<u8>;
  // Read and parse the headers.
  let mut length_string: [u8, ..10] = unsafe {std::mem::uninitialized()};
  let mut length_string_len = 0u;
  loop {
    let ch = try! (stream.read_char());
    if ch >= '0' && ch <= '9' {
      length_string[length_string_len] = ch as u8; length_string_len += 1;
    } else if ch == ':' {
      let length_str = try! (from_utf8 (length_string[..length_string_len]) .ok_or (BadLength));
      let length: uint = try! (from_str (length_str) .ok_or (BadLength));
      let headers_buf = try! (stream.read_exact (length));
      if try! (stream.read_char()) != ',' {return Err (WrongLength (length_str.to_string()))}
      headers = headers_buf; break;
    } else {
      length_string[length_string_len] = ch as u8; length_string_len += 1;
      return Err (WrongLength (try! (from_utf8 (length_string[..length_string_len]) .ok_or (BadLength)).to_string()));
    }
  };
  let mut pos = 0u;
  while pos < headers.len() {
    let zero1 = try! (headers[pos..].iter().position (|&ch|ch == 0) .ok_or (WrongHeaders));
    let header_name = try! (from_utf8 (headers[pos .. pos + zero1]) .ok_or (WrongHeaders));
    pos = pos + zero1 + 1;
    let zero2 = try! (headers[pos..].iter().position (|&ch|ch == 0) .ok_or (WrongHeaders));
    let header_value = try! (from_utf8 (headers[pos .. pos + zero2]) .ok_or (WrongHeaders));
    header (header_name, header_value);
    pos = pos + zero2 + 1;
  }
  Ok (stream)
}

/// Parse the headers and pack them into a map.
///
/// Returns the map with the headers and the `tcp_stream` wrapped into a `BufferedStream`.<br>
/// You should use the stream to read the rest of the query and send the response.
pub fn scgi_string_map (tcp_stream: TcpStream) -> Result<(HashMap<String, String, FnvHasher>, BufferedStream<TcpStream>), ScgiError> {
  let mut headers_map = std::collections::HashMap::with_capacity_and_hasher (48, FnvHasher);
  let buffered_stream = try! (scgi_parse (tcp_stream, |name,value| {headers_map.insert (name.to_string(), value.to_string());}));
  Ok ((headers_map, buffered_stream))
}

#[test] fn test_scgi() {
  let port = 13123;
  spawn (proc() {
    sleep (Duration::milliseconds (10));
    let mut stream = TcpStream::connect (("127.0.0.1", port));
    stream.write (b"70:CONTENT_LENGTH\x0056\x00SCGI\x001\x00REQUEST_METHOD\x00POST\x00REQUEST_URI\x00/deepthought\x00,") .unwrap();
    stream.write (b"What is the answer to life, the Universe and everything?") .unwrap();
    assert_eq! (stream.read_to_string().unwrap()[], "Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42");
  });
  let mut acceptor = TcpListener::bind (("127.0.0.1", port)) .unwrap().listen().unwrap();
  acceptor.set_timeout (Some (100));
  let stream = acceptor.incoming().next().unwrap();
  match stream {
    Err (err) => {panic! ("Accept error: {}", err)},
    Ok (tcp_stream) => {
      let (map, mut stream) = scgi_string_map (tcp_stream) .unwrap();
      assert_eq! (map["REQUEST_URI".to_string()][], "/deepthought");
      stream.write (b"Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42") .unwrap();
    }
  }
}
