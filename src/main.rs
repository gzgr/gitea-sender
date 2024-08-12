use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use env_logger;
use log::info;
use serde::Serialize;
use serde_json::Value;
use std::process::Command;
use std::path::Path;

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
            info!("Repository updated successfully:\n{}", String::from_utf8_lossy(&output.stdout));
        } else {
            info!("Failed to update repository:\n{}", String::from_utf8_lossy(&output.stderr));
        }
    } else {
        info!("Repository path does not exist: {}", repo_path);
    }

    // Webhook으로 전달된 JSON 데이터를 파싱하여 필요한 값을 추출
    if let Some(commits) = payload.get("commits").and_then(|c| c.as_array()) {
        for commit in commits {
            if let Some(added_files) = commit.get("added").and_then(|a| a.as_array()) {
                let added_files_vec: Vec<String> = added_files
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();

                // 추출한 벡터를 로그로 출력
                info!("Added files: {:?}", added_files_vec);
            }
        }
    }

    // 요청에 대해 HTTP 200 OK 응답
    HttpResponse::Ok().body("Webhook processed")
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
