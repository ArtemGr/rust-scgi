//! SCGI parser.
//!
//! This is partly a port of my Java parser: https://gist.github.com/ArtemGr/38425.
// [build] cd .. && cargo test

extern crate bufstream;

use bufstream::BufStream;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::io;
use std::io::{Read, Write};
#[cfg(test)] use std::net::{TcpStream, TcpListener};
use std::str::{from_utf8, Utf8Error};

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
  /// No more data. The socket has been closed prematurely.
  EOF,
  /// IoError, like when connection closed prematurely.
  IO (io::Error)
}
impl From<io::Error> for ScgiError {fn from (io_error: io::Error) -> ScgiError {IO (io_error)}}
impl From<Utf8Error> for ScgiError {fn from (utf8_error: Utf8Error) -> ScgiError {Utf8 (utf8_error)}}
impl Display for ScgiError {
  fn fmt (&self, fmt: &mut Formatter) -> Result<(), std::fmt::Error> {write! (fmt, "{:?}", self)}
}

impl std::error::Error for ScgiError {
    fn description(&self) -> &str {
        "ScgiError"
    }
    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Utf8(ref e) => Some(e),
            IO(ref e) => Some(e),
            _ => None,
        }
    }
}

/// Read the headers from the stream.
///
/// Returns the vector containing the headers and the `tcp_stream` wrapped into a `BufferedStream`.<br>
/// You should use the stream to read the rest of the query and send the response.
pub fn read_headers<S: Read + Write> (stream: S) -> Result<(Vec<u8>, BufStream<S>), ScgiError> {
  let mut stream = BufStream::new (stream);
  let raw_headers: Vec<u8>;
  // Read the headers.
  let mut length_string: [u8; 10] = unsafe {std::mem::uninitialized()};
  let mut length_string_len = 0usize;
  loop {
    let mut ch_buf = [0u8];
    let got = try! (stream.read (&mut ch_buf));
    if got == 0 {return Err (EOF)}
    let ch = ch_buf[0];
    if ch >= b'0' && ch <= b'9' {
      length_string[length_string_len] = ch; length_string_len += 1;
    } else if ch == b':' {
      let length_str = try! (from_utf8 (&length_string[0 .. length_string_len]));
      let length: usize = try! (length_str.parse().map_err (|_| BadLength));
      let mut headers_buf = Vec::with_capacity (length);
      unsafe {headers_buf.set_len (length)}
      let mut total = 0;
      while total < length {
        let got = try! (stream.read (&mut headers_buf[total .. length]));
        if got == 0 {return Err (EOF)}
        total += got
      }
      if try! (stream.read (&mut ch_buf)) != 1 || ch_buf[0] != b',' {return Err (WrongLength (length_str.to_string()))}
      raw_headers = headers_buf; break;
    } else {
      length_string[length_string_len] = ch; length_string_len += 1;
      return Err (WrongLength (try! (from_utf8 (&length_string[0 .. length_string_len])).to_string()));
    }
  };
  Ok ((raw_headers, stream))
}

/// Parse the headers, invoking the `header` closure for every header parsed.
pub fn parse<'h,H> (raw_headers: &'h [u8], mut header: H) -> Result<(), ScgiError> where H: FnMut(&'h str,&'h str) {
  let mut pos = 0usize;
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
pub fn string_map (raw_headers: &[u8]) -> Result<BTreeMap<String, String>, ScgiError> {
  let mut headers_map = BTreeMap::new();
  try! (parse (raw_headers, |name,value| {headers_map.insert (name.to_string(), value.to_string());}));
  Ok (headers_map)
}

/// Parse the headers and pack them as slices into a map.
pub fn str_map<'h> (raw_headers: &'h [u8]) -> Result<BTreeMap<&'h str, &'h str>, ScgiError> {
  let mut headers_map = BTreeMap::new();
  try! (parse (raw_headers, |name,value| {headers_map.insert (name, value);}));
  Ok (headers_map)
}

#[test] fn test_scgi() {
  let port = 13123;
  std::thread::spawn (move|| {
    std::thread::sleep_ms (10);
    let mut stream = TcpStream::connect (("127.0.0.1", port)) .unwrap();
    stream.write (b"70:CONTENT_LENGTH\x0056\x00SCGI\x001\x00REQUEST_METHOD\x00POST\x00REQUEST_URI\x00/deepthought\x00,") .unwrap();
    stream.write (b"What is the answer to life, the Universe and everything?") .unwrap();
    let mut buf = String::new();
    stream.read_to_string (&mut buf).unwrap();
    assert_eq! (&buf[..], "Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42");
  });
  let acceptor = TcpListener::bind (("127.0.0.1", port)) .unwrap();
  let stream = acceptor.incoming().next().unwrap();
  match stream {
    Err (err) => {panic! ("Accept error: {}", err)},
    Ok (tcp_stream) => {
      let (raw_headers, mut stream) = read_headers (tcp_stream) .unwrap();
      assert_eq! (str_map (&raw_headers) .unwrap() ["REQUEST_URI"], "/deepthought");
      assert_eq! (&(string_map (&raw_headers) .unwrap() ["REQUEST_URI"])[..], "/deepthought");
      stream.write_all (b"Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42") .unwrap();
    }
  }
}
