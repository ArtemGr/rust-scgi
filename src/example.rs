#![feature(if_let)]
#![feature(unboxed_closures)]
#![feature(overloaded_calls)]

extern crate collections;
extern crate rustc;
extern crate serialize;
extern crate scgi;

use std::io::net::tcp::{TcpListener};
use std::io::{Listener, Acceptor};

pub fn main() {
  fn accept_scgi() -> Result<(), scgi::ScgiError> {
    let mut acceptor = try! (TcpListener::bind (("127.0.0.1", 8083)) .listen());
    for stream in acceptor.incoming() {
      match stream {
        Err (err) => println! ("scgi] Accept error: {}", err),
        Ok (tcp_stream) => spawn (proc() {
          if let Err (error) = (move |:| -> Result<(), scgi::ScgiError> {
            let (raw_headers, mut stream) = try! (scgi::read_headers (tcp_stream));
            let headers_map = try! (scgi::str_map (&raw_headers));
            let uri = headers_map["REQUEST_URI"];
            println! ("scgi] Serving uri '{}'.", uri);

            try! (stream.write (
              b"Status: 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\nHi\r\n"));
            Ok(())
          })() {println! ("scgi] Error: {}", error);}
        })
      }
    }
    Ok(())  // This line is never reached.
  }
  if let Err (error) = accept_scgi() {println! ("scgi] Outer error: {}", error)}
}
