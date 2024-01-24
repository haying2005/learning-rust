use tokio::io::{ AsyncBufReadExt, AsyncWriteExt, self, AsyncReadExt };
use tokio::net::{ TcpStream, TcpListener };
#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:8888").await.unwrap();
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();

        // let (mut r, mut w) = socket.split();
        // io::copy(&mut r, &mut w).await.unwrap();

        tokio::spawn(async move {
            // note: 使用堆数组而不是栈数组，防止task结构体过大；因为栈数组会被存储在task结构体中(stored inline)
            let mut buf = vec![0; 1024];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        println!("end of file");
                        return;
                    },
                    Ok(n) => socket.write_all(&buf[0.. n]).await.unwrap(),
                    Err(_) => return,
                }
            };
        });
    }
}