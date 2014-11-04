// SCGI server and request dispatcher.

#![feature(slicing_syntax)]

use std::collections::hashmap::HashMap;
use std::io::{Listener, Acceptor, BufferedStream};
use std::io::net::tcp::{TcpListener, TcpStream};
use std::io::timer::sleep;
use std::str::from_utf8;
use std::time::duration::Duration;

pub fn scgi_parse (tcp_stream: TcpStream, header: |&str,&str|) -> BufferedStream<TcpStream> {
  let mut stream = BufferedStream::new (tcp_stream);
  let mut headers: Vec<u8>;
  // Read and parse the headers.
  let mut length_string: [u8, ..10] = unsafe {std::mem::uninitialized()};
  let mut length_string_len = 0u;
  loop {
    let ch = stream.read_char().unwrap();
    if ch >= '0' && ch <= '9' {
      length_string[length_string_len] = ch as u8; length_string_len += 1;
    } else if ch == ':' {
      let length: uint = from_str (from_utf8 (length_string[..length_string_len]) .unwrap()) .unwrap();
      let headers_buf = stream.read_exact (length) .unwrap();
      if stream.read_char().unwrap() != ','
        {panic! ("Wrong SCGI header length: {}", from_utf8 (length_string[..length_string_len]) .unwrap());}
      headers = headers_buf; break;
    } else {
      length_string[length_string_len] = ch as u8; length_string_len += 1;
      panic! ("Wrong SCGI header length: {}", from_utf8 (length_string[..length_string_len]) .unwrap());
    }
  };
  let mut pos = 0u;
  while pos < headers.len() {
    let zero1 = headers[pos..].iter().position (|&ch|ch == 0) .unwrap();
    let header_name = from_utf8 (headers[pos .. pos + zero1]) .unwrap();
    pos = pos + zero1 + 1;
    let zero2 = headers[pos..].iter().position (|&ch|ch == 0) .unwrap();
    let header_value = from_utf8 (headers[pos .. pos + zero2]) .unwrap();
    header (header_name, header_value);
    pos = pos + zero2 + 1;
  }
  stream
}

/*pub fn scgi_map (tcp_stream: TcpStream) {
        let mut uri: &str = "";
        let mut headers_map = HashMap::<&str, &str>::with_capacity (48);
            if header_name == "REQUEST_URI" {uri = header_value}
            //status.log (format! ("{}: {}", header_name, header_value));
            headers_map.insert (header_name, header_value);
}*/

#[test] fn test_scgi() {
  let port = 13123;
  spawn (proc() {
    sleep (Duration::milliseconds (10));
    let mut stream = TcpStream::connect ("127.0.0.1", port);
    stream.write (b"70:CONTENT_LENGTH\x0056\x00SCGI\x001\x00REQUEST_METHOD\x00POST\x00REQUEST_URI\x00/deepthought\x00,") .unwrap();
    stream.write (b"What is the answer to life, the Universe and everything?") .unwrap();
    assert_eq! (stream.read_to_string().unwrap()[], "Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42");
  });
  let mut acceptor = TcpListener::bind ("127.0.0.1", port) .listen().unwrap();
  acceptor.set_timeout (Some (100));
  let stream = acceptor.incoming().next().unwrap();
  match stream {
    Err (err) => {panic! ("Accept error: {}", err)},
    Ok (tcp_stream) => {
      let mut stream = scgi_parse (tcp_stream, |_,_|{});
      stream.write (b"Status: 200 OK\r\nContent-Type: text/plain\r\n\r\n42") .unwrap();
    }
  }
}
