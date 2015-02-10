//! SCGI parser.
//!
//! This is partly a port of my Java parser: https://gist.github.com/ArtemGr/38425.
// [build] cd .. && cargo test

#![feature(io,core,std_misc)]

//use rustc::util::nodemap::FnvHasher;  // http://www.reddit.com/r/rust/comments/2l4kxf/std_hashmap_is_slow/
use std::collections::HashMap;
use std::error::FromError;
use std::old_io::{BufferedStream, IoError};
use std::old_io::net::tcp::{TcpStream};
use std::str::{from_utf8, Utf8Error};

#[cfg(test)] use std::old_io::{Listener, Acceptor};
#[cfg(test)] use std::old_io::net::tcp::{TcpListener};
#[cfg(test)] use std::old_io::timer::sleep;
#[cfg(test)] use std::thread::Thread;
#[cfg(test)] use std::time::duration::Duration;

use ScgiError::*;

/// SCGI parsing errors.
#[derive(Debug)]
pub enum ScgiError {
  /// Length can't be decoded to an integer.
  BadLength,
  /// The length or the headers are not in UTF-8.
  Utf8 (Utf8Error),
  /// Netstring sanity checks fail.
  WrongLength (String),
  /// Error parsing the zero-terminated HTTP headers.
  WrongHeaders,
  /// IoError, like when connection closed prematurely.
  IO (IoError)
}
impl FromError<IoError> for ScgiError {fn from_error (io_error: IoError) -> ScgiError {IO (io_error)}}
impl FromError<Utf8Error> for ScgiError {fn from_error (utf8_error: Utf8Error) -> ScgiError {Utf8 (utf8_error)}}

/// Read the headers from the stream.
///
/// Returns the vector containing the headers and the `tcp_stream` wrapped into a `BufferedStream`.<br>
/// You should use the stream to read the rest of the query and send the response.
pub fn read_headers (tcp_stream: TcpStream) -> Result<(Vec<u8>, BufferedStream<TcpStream>), ScgiError> {
  let mut stream = BufferedStream::new (tcp_stream);
  let mut raw_headers: Vec<u8>;
  // Read the headers.
  let mut length_string: [u8; 10] = unsafe {std::mem::uninitialized()};
  let mut length_string_len = 0us;
  loop {
    let ch = try! (stream.read_char());
    if ch >= '0' && ch <= '9' {
      length_string[length_string_len] = ch as u8; length_string_len += 1;
    } else if ch == ':' {
      let length_str = try! (from_utf8 (&length_string[0 .. length_string_len]));
      let length: usize = try! (length_str.parse().map_err (|_| BadLength));
      let headers_buf = try! (stream.read_exact (length));
      if try! (stream.read_char()) != ',' {return Err (WrongLength (length_str.to_string()))}
      raw_headers = headers_buf; break;
    } else {
      length_string[length_string_len] = ch as u8; length_string_len += 1;
      return Err (WrongLength (try! (from_utf8 (&length_string[0 .. length_string_len])).to_string()));
    }
  };
  Ok ((raw_headers, stream))
}

/// Parse the headers, invoking the `header` closure for every header parsed.
pub fn parse<'h,H> (raw_headers: &'h Vec<u8>, mut header: H) -> Result<(), ScgiError> where H: FnMut(&'h str,&'h str) {
  let mut pos = 0us;
  while pos < raw_headers.len() {
    let zero1 = try! (raw_headers[pos..].iter().position (|&ch|ch == 0) .ok_or (WrongHeaders));
    let header_name = try! (from_utf8 (&raw_headers[pos .. pos + zero1]));
    pos = pos + zero1 + 1;
    let zero2 = try! (raw_headers[pos..].iter().position (|&ch|ch == 0) .ok_or (WrongHeaders));
    let header_value = try! (from_utf8 (&raw_headers[pos .. pos + zero2]));
    header (header_name, header_value);
    pos = pos + zero2 + 1;
  }
  Ok(())
}

/// Parse the headers and pack them as strings into a map.
pub fn string_map (raw_headers: &Vec<u8>) -> Result<HashMap<String, String>, ScgiError> {
  let mut headers_map = std::collections::HashMap::with_capacity (48);
  try! (parse (raw_headers, |name,value| {headers_map.insert (name.to_string(), value.to_string());}));
  Ok (headers_map)
}

/// Parse the headers and pack them as slices into a map.
pub fn str_map<'h> (raw_headers: &'h Vec<u8>) -> Result<HashMap<&'h str, &'h str>, ScgiError> {
  let mut headers_map = std::collections::HashMap::with_capacity (48);
  try! (parse (raw_headers, |name,value| {headers_map.insert (name, value);}));
  Ok (headers_map)
}

#[test] fn test_scgi() {
  let port = 13123;
  Thread::spawn (move|| {
    sleep (Duration::milliseconds (10));
    let mut stream = TcpStream::connect (("127.0.0.1", port));
    stream.write_all (b"70:CONTENT_LENGTH\x0056\x00SCGI\x001\x00REQUEST_METHOD\x00POST\x00REQUEST_URI\x00/deepthought\x00,") .unwrap();
    stream.write_all (b"What is the answer to life, the Universe and everything?") .unwrap();
    assert_eq! (stream.read_to_string().unwrap().as_slice(), "Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42");
  });
  let mut acceptor = TcpListener::bind (("127.0.0.1", port)) .unwrap().listen().unwrap();
  acceptor.set_timeout (Some (100));
  let stream = acceptor.incoming().next().unwrap();
  match stream {
    Err (err) => {panic! ("Accept error: {}", err)},
    Ok (tcp_stream) => {
      let (raw_headers, mut stream) = read_headers (tcp_stream) .unwrap();
      assert_eq! (str_map (&raw_headers) .unwrap() ["REQUEST_URI"], "/deepthought");
      assert_eq! (string_map (&raw_headers) .unwrap() ["REQUEST_URI".to_string()] .as_slice(), "/deepthought");
      stream.write_all (b"Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42") .unwrap();
    }
  }
}
