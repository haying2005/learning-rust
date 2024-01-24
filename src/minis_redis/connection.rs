use std::{error::Error, io::Cursor, ops::Deref};


use mini_redis::{ Result, frame::{ self, Frame } };
use tokio::{net::{ TcpStream }, io::{self, AsyncReadExt, BufWriter, AsyncWriteExt}};
use bytes::{self, BytesMut, BufMut, Buf};

pub struct Connection {
    // stream: TcpStream,
    /// 缓冲写: 为减少syscall, 会把数据写入内部缓冲；
    /// 但是有些情况会绕过缓冲直接写入socket,例如数据量较大的情况,因为复制数据到缓冲耗费性能
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(stream),
            buffer: BytesMut::with_capacity(1024),
        }
    }

    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        // Arrays are encoded by encoding each entry. All other frame types are
        // considered literals. For now, mini-redis is not able to encode
        // recursive frame structures. See below for more details.
        match frame {
            Frame::Array(val) => {
                // Encode the frame type prefix. For an array, it is `*`.
                self.stream.write_u8(b'*').await?;

                // Encode the length of the array.
                self.write_decimal(val.len() as u64).await?;

                // Iterate and encode each entry in the array.
                for entry in &**val {
                    self.write_value(entry).await?;
                }
            }
            // The frame type is a literal. Encode the value directly.
            _ => self.write_value(frame).await?,
        }

        // Ensure the encoded frame is written to the socket. The calls above
        // are to the buffered stream and writes. Calling `flush` writes the
        // remaining contents of the buffer to the socket.
        self.stream.flush().await
    }

    pub async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            if let Some(frame) = self.parse_frame()? {
                return Ok(Some(frame))
            }

            // 不停的从socket中读取数据，直到能返回一个成功解析的frame 或者 连接断开
            // note:用read_buf不用read, 因为read_buf方法内部会advancing the buffer's internal cursor
            let n = self.stream.read_buf(&mut self.buffer).await?;
            if n == 0 { // end of file: tcp连接断开
                if self.buffer.len() > 0 {
                    return Err("Conn Reset By Peer".into())
                } else {
                    // peer closed normally
                    return Ok(None)
                }
            }
        }
    }

    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        let mut buf = Cursor::new(&self.buffer[..]);
        match Frame::check(&mut buf) {
            Err(frame::Error::Incomplete) => Ok(None),
            Err(e) => Err(e.into()),
            Ok(()) => {
                let len = buf.position(); // 一个完整frame的长度
                buf.set_position(0); // reset the position to 0, 因为check操作内部会advance cursor
                let frame = Frame::parse(&mut buf)?;
                self.buffer.advance(len as usize); // 丢弃掉buffer中解析过的字节
                Ok(Some(frame))
            }
        }
    }
    /// Write a frame literal to the stream
    async fn write_value(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Simple(val) => {
                self.stream.write_u8(b'+').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Error(val) => {
                self.stream.write_u8(b'-').await?;
                self.stream.write_all(val.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Integer(val) => {
                self.stream.write_u8(b':').await?;
                self.write_decimal(*val).await?;
            }
            Frame::Null => {
                self.stream.write_all(b"$-1\r\n").await?;
            }
            Frame::Bulk(val) => {
                let len = val.len();

                self.stream.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.stream.write_all(val).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            // Encoding an `Array` from within a value cannot be done using a
            // recursive strategy. In general, async fns do not support
            // recursion. Mini-redis has not needed to encode nested arrays yet,
            // so for now it is skipped.
            Frame::Array(_val) => unreachable!(),
        }

        Ok(())
    }

    /// Write a decimal frame to the stream
    async fn write_decimal(&mut self, val: u64) -> io::Result<()> {
        use std::io::Write;

        // Convert the value to a string
        let mut buf = [0u8; 12];
        let mut buf = Cursor::new(&mut buf[..]);
        write!(&mut buf, "{}", val)?;

        let pos = buf.position() as usize;
        self.stream.write_all(&buf.get_ref()[..pos]).await?;
        self.stream.write_all(b"\r\n").await?;

        Ok(())
    }
}