use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let mut f = File::open("hello.txt").await?;
    
    let n = f.write(b"xxxxxx").await?;
    println!("write bytes {}", n);
    
    let mut buf = Vec::new();
    let n = f.read_to_end(&mut buf).await?;
    println!("read bytes {}", n);
    println!("{}", String::from_utf8(buf).unwrap());

    Ok(())
}