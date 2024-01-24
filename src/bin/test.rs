use std::{io::{Cursor, Write, Read, Error, self}, net::TcpStream, cell::RefCell, thread};

use bytes::{Bytes, Buf, BytesMut, BufMut};
use tokio::io::{BufWriter, AsyncRead};


fn main() {
    let mut buf = Bytes::from(&b"aaaaaa"[..]);
    let x = RefCell::new(buf);
    let xxxx = x.borrow_mut();

    let xx2 = x.borrow();

    let xx3 = x.borrow_mut();
    // drop(xxxx);
    // thread::spawn(move || {
    //     let xx = &*x.borrow_mut();
    //     println!("xxx{:?}", xx);
    // });
    

}
fn string_uppercase(mut data: &TcpStream) {
    let s = "x".to_owned();
    // if let Err(e) = data.write(buf) {
    //     e.kind() == io::ErrorKind::Interrupted
    // }
}