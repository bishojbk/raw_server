use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;



fn build_response(status_line: &str, body: &str) -> String {
    format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        body.len(),
        body
    )
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("listening on http://127.0.0.1:8080");

    loop {
        let (mut stream, addr) = listener.accept().await?;
        println!("new connection from {}", addr);

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let n = match stream.read(&mut buf).await {
                Ok(0) => return,                 // client closed
                Ok(n) => n,
                Err(e) => { eprintln!("read error: {}", e); return; }
            };

            let request_text = String::from_utf8_lossy(&buf[..n]);
            let request_line = request_text.lines().next().unwrap_or("");
            let mut parts = request_line.split(' ');
            let _method = parts.next().unwrap_or("");
            let path = parts.next().unwrap_or("");

            let response = match path {
                "/" => build_response("HTTP/1.1 200 OK", "hello from the server\n"),
                "/health" => build_response("HTTP/1.1 200 OK", "ok\n"),
                "/slow" => {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    build_response("HTTP/1.1 200 OK", "slow response\n")
                }
                _ => build_response("HTTP/1.1 404 Not Found", "not found\n"),
            };

            if let Err(e) = stream.write_all(response.as_bytes()).await {
                eprintln!("write error: {}", e);
            }
        });
    }
}