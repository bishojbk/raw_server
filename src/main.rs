use std::io::{Read, Write};
use std::net::TcpListener;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080")?;
    println!("listening on http://127.0.0.1:8080");

    for incoming in listener.incoming() {
        let mut stream = incoming?;
        println!("new connection from {}", stream.peer_addr()?);

        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf)?;
        println!("read {} bytes:", n);
        println!("---");
        println!("{}", String::from_utf8_lossy(&buf[..n]));
        println!("---");

        let body = "hello from the server\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes())?;
    }

    Ok(())
}