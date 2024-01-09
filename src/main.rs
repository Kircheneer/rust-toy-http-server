use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug)]
struct HTTPRequest<'a> {
    method: &'a str,
    path: &'a str,
    version: &'a str,
    headers: HashMap<String, String>,
    body: Vec<&'a str>,
}

#[derive(Debug)]
struct HTTPResponse<'a> {
    http_version: &'a str,
    status_code: u8,
    reason_phrase: &'a str,
    headers: HashMap<String, String>,
    body: Vec<&'a str>,
}

impl HTTPResponse<'_> {
    fn serialize(&self) -> String {
        let mut serialized = format!(
            "{} {}",
            self.http_version, self.status_code
        );
        if !self.reason_phrase.is_empty() {
            serialized.push_str(&*format!(" {}", self.reason_phrase));
        }
        serialized.push_str("\r\n");
        for (name, value) in self.headers.iter() {
            serialized.push_str(&*format!("{}: {}\r\n", name, value));
        }
        for line in self.body.iter() {
            serialized.push_str(line);
        }
        serialized.push_str("\r\n");

        serialized
    }
}

async fn parse_request(buf: &[u8; 1024]) -> Result<HTTPRequest, Box<dyn std::error::Error>> {
    let parsed = std::str::from_utf8(&buf[0..1023])?;
    let mut lines = parsed.lines();
    let mut first_line_split = lines
        .next()
        .ok_or("Failed parsing request line")?
        .split_ascii_whitespace();
    let method = first_line_split.next().ok_or("Failed parsing method")?;
    let path = first_line_split.next().ok_or("Failed parsing path")?;
    let version = first_line_split.next().ok_or("Failed parsing version")?;
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let line = lines.next().ok_or("Missing request body")?;
        if line.trim().len() == 0 {
            break;
        }
        let (header_name, header_value) = line.split_once(' ').ok_or("Failed parsing headers.")?;
        headers.insert(header_name.to_string(), header_value.to_string());
    }
    let body = lines.collect::<Vec<&str>>();

    Ok(HTTPRequest {
        method,
        path,
        version,
        headers,
        body,
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buf: [u8; 1024] = [0; 1024];

            // In a loop, read data from the socket and write the data back.
            loop {
                let n = match socket.read(&mut buf).await {
                    // socket closed
                    Ok(n) if n == 0 => return,
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };

                let request = match parse_request(&buf).await {
                    Ok(request) => request,
                    Err(e) => {
                        eprintln!("failed parsing HTTP packet; err = {:?}", e);
                        return;
                    }
                };

                let mut response_headers = HashMap::new();
                response_headers.insert("Content-Length".to_owned(), "0".to_owned());

                let response = HTTPResponse {
                    http_version: "HTTP/1.1",
                    status_code: 200,
                    reason_phrase: "OK",
                    headers: response_headers,
                    body: Vec::new(),
                }.serialize();

                eprintln!("{}", response);

                // Write the data back
                if let Err(e) = socket.write_all(response.as_bytes()).await {
                    eprintln!("failed to write to socket; err = {:?}", e);
                    return;
                }
            }
        });
    }
}
