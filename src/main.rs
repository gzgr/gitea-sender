use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use env_logger;
use ftp::FtpStream;
use log::info;
use serde::Serialize;
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use zip::write::FileOptions;
use std::collections::HashMap;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

async fn health() -> impl Responder {
    let response = HealthResponse {
        status: "ok".to_string(),
    };
    HttpResponse::Ok().json(response)
}

async fn webhook_handler(payload: web::Json<Value>) -> impl Responder {
    info!("Received webhook request");

    // 특정 경로에 있는 git 저장소를 최신 상태로 업데이트
    let repo_path = "./test/webhook-test"; // 여기에 실제 경로를 입력하세요

    if Path::new(repo_path).exists() {
        info!("Updating repository at {}", repo_path);

        let output = Command::new("git")
            .arg("-C")
            .arg(repo_path)
            .arg("pull")
            .output()
            .expect("Failed to execute git pull");

        if output.status.success() {
            info!(
                "Repository updated successfully:\n{}",
                String::from_utf8_lossy(&output.stdout)
            );
        } else {
            info!(
                "Failed to update repository:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    } else {
        info!("Repository path does not exist: {}", repo_path);
    }

    // 폴더별로 변경된 파일들을 그룹화하여 ZIP으로 압축
    let mut folders_to_files: HashMap<String, Vec<String>> = HashMap::new();

    if let Some(commits) = payload.get("commits").and_then(|c| c.as_array()) {
        for commit in commits {
            if let Some(added_files) = commit.get("added").and_then(|a| a.as_array()) {
                for file in added_files {
                    if let Some(file_path) = file.as_str() {
                        let full_path = format!("{}/{}", repo_path, file_path);

                        // 파일 경로에서 폴더 경로를 추출
                        if let Some(folder_path) = Path::new(file_path).parent() {
                            let folder_str = folder_path.to_string_lossy().to_string();
                            folders_to_files
                                .entry(folder_str)
                                .or_insert_with(Vec::new)
                                .push(full_path);
                        }
                    }
                }
            }
        }

        // 각 폴더에 대해 ZIP 파일 생성
        for (folder, files) in folders_to_files.iter() {
            let zip_file_name = folder.replace("/", "_"); // 폴더 이름을 ZIP 파일명으로 사용
            let zip_file_path = format!("./{}.zip", zip_file_name);

            let zip_file = File::create(&zip_file_path).expect("Could not create ZIP file");
            let mut zip = zip::ZipWriter::new(zip_file);

            for file_path in files {
                let file_name_in_zip = Path::new(file_path)
                    .strip_prefix(repo_path)
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                if Path::new(file_path).exists() {
                    let options =
                        FileOptions::default().compression_method(zip::CompressionMethod::Stored);
                    zip.start_file(&file_name_in_zip, options)
                        .expect("Could not start file in ZIP");

                    let mut file =
                        File::open(file_path).expect("Could not open file to add to ZIP");
                    let mut buffer = Vec::new();
                    file.read_to_end(&mut buffer).expect("Could not read file");
                    zip.write_all(&buffer).expect("Could not write file to ZIP");
                }
            }

            zip.finish().expect("Could not finish ZIP file");
            info!("Created ZIP file at {}", zip_file_path);

            // ZIP 파일을 FTP 서버로 전송
            let ftp_server = "ftp.example.com"; // FTP 서버 주소
            let ftp_username = "your_username"; // FTP 사용자명
            let ftp_password = "your_password"; // FTP 비밀번호
            let remote_path = format!("/remote/path/{}.zip", zip_file_name); // FTP 서버에 저장할 경로

            // match send_via_ftp(
            //     ftp_server,
            //     ftp_username,
            //     ftp_password,
            //     &zip_file_path,
            //     &remote_path,
            // ) {
            //     Ok(_) => info!("Successfully uploaded ZIP file to FTP server"),
            //     Err(e) => info!("Failed to upload ZIP file to FTP server: {}", e),
            // }
        }
    }

    // 요청에 대해 HTTP 200 OK 응답
    HttpResponse::Ok().body("Webhook processed")
}

fn send_via_ftp(
    server: &str,
    username: &str,
    password: &str,
    local_file_path: &str,
    remote_file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ftp_stream = FtpStream::connect(server)?;
    ftp_stream.login(username, password)?;

    // 파일 전송을 위한 바이너리 모드 전환
    ftp_stream.transfer_type(ftp::types::FileType::Binary)?;

    // 로컬 파일을 읽어 FTP 서버에 업로드
    let mut file = File::open(local_file_path)?;
    ftp_stream.put(remote_file_path, &mut file)?;

    // FTP 세션 종료
    ftp_stream.quit()?;
    Ok(())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 로깅 초기화 및 로그 레벨 설정
    env_logger::Builder::from_default_env()
        .filter(None, log::LevelFilter::Info)
        .init();

    HttpServer::new(|| {
        App::new()
            .route("/health", web::get().to(health))
            .route("/webhook", web::post().to(webhook_handler))
    })
    .bind("0.0.0.0:10080")?
    .run()
    .await
}
