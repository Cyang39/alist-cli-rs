use reqwest::{
    header::{HeaderMap, AUTHORIZATION},
    Client,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use std::process;
use url::Url;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
use futures_util::stream::StreamExt; // 用于处理异步流

#[derive(Serialize, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct LoginResponse {
    code: u16,
    message: String,
    data: Option<LoginData>, // data 是可选的，因为可能会有错误
}

#[derive(Serialize, Deserialize, Debug)]
struct LoginData {
    token: String,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 7 {
        eprintln!(
            "Usage: {} --username <username> --password <password> <local-file> <alist-url>",
            args[0]
        );
        process::exit(1);
    }

    let mut args_iter = args.iter();
    args_iter.next();

    let username = match args_iter.find(|x| *x == "--username") {
        Some(_) => args_iter.next().unwrap(),
        None => {
            eprintln!("Error: --username flag is required");
            process::exit(1);
        }
    };

    let password = match args_iter.find(|&x| x == "--password") {
        Some(_) => args_iter.next().unwrap(),
        None => {
            eprintln!("Error: --password flag is required");
            process::exit(1);
        }
    };

    let local_file = match args_iter.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Error: Local file path is required");
            process::exit(1);
        }
    };

    let remote_path = match args_iter.next() {
        Some(arg) => arg,
        None => {
            eprintln!("Error: Remote path is required");
            process::exit(1);
        }
    };

    let base_url = match Url::parse(remote_path) {
        Ok(parsed_url) => parsed_url[..url::Position::BeforePath].to_string(),
        Err(_) => {
            eprintln!("Error: Invalid URL");
            process::exit(1);
        }
    };

    // Get Token: https://alist.nn.ci/guide/api/fs.html#put-流式上传文件
    let client = Client::new();
    let login_url = format!("{}/api/auth/login", base_url);
    let login_response = client
        .post(&login_url)
        .json(&LoginRequest {
            username: username.clone(),
            password: password.clone(),
        })
        .send()
        .await
        .expect("Failed to send login request");

    let text_response = login_response
        .text()
        .await
        .expect("Failed to parse login response");

    let parsed_response: LoginResponse =
        serde_json::from_str(&text_response).expect("Failed to deserialize response");

    let token;

    if parsed_response.message == "success" {
        if let Some(data) = parsed_response.data {
            token = data.token;
        } else {
            eprintln!("Error: No token received in response data");
            std::process::exit(1);
        }
    } else {
        eprintln!("Login failed with message: {}", parsed_response.message);
        std::process::exit(1);
    }

    // Upload File: https://alist.nn.ci/guide/api/fs.html#put-流式上传文件
    let remote_file_path = remote_path.replace(&base_url, "");
    let upload_url = format!("{}/api/fs/put", base_url);
    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, format!("{}", token).parse().unwrap());
    headers.insert("File-Path", remote_file_path.parse().unwrap());

    let file = File::open(local_file).await.expect("Failed to open file");
    // 将文件转换为异步字节流
    let file_stream = FramedRead::new(file, BytesCodec::new())
        .map(|result| result.map(|bytes| bytes.freeze()));

    let body = reqwest::Body::wrap_stream(file_stream);

    let upload_response = client
        .put(&upload_url)
        .headers(headers)
        .body(body)
        .send()
        .await
        .expect("Failed to send upload request");

    let text_response = upload_response
        .text()
        .await
        .expect("Failed to parse login response");

    println!("Upload response: {:?}", text_response);
}
