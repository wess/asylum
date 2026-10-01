//! Dictation and voice memos against a fake xAI speech API.

use axum::extract::Multipart;
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine;
use serde_json::{json, Value};

async fn server() -> String {
  let app = Router::new()
    .route(
      "/v1/stt",
      post(|mut form: Multipart| async move {
        let mut bytes = 0;
        let mut lang = String::new();
        while let Ok(Some(field)) = form.next_field().await {
          let name = field.name().unwrap_or("").to_string();
          let data = field.bytes().await.unwrap();
          if name == "file" {
            bytes = data.len();
          } else if name == "language" {
            lang = String::from_utf8_lossy(&data).into();
          }
        }
        Json(json!({ "text": format!("heard {bytes} bytes in {lang}") }))
      }),
    )
    .route(
      "/v1/tts",
      post(|Json(body): Json<Value>| async move {
        let audio = base64::engine::general_purpose::STANDARD.encode(format!("mp3:{}:{}", body["voice_id"].as_str().unwrap(), body["text"].as_str().unwrap()));
        Json(json!({ "audio": audio, "content_type": "audio/mpeg" }))
      }),
    );
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  format!("http://{addr}/v1")
}

#[tokio::test(flavor = "multi_thread")]
async fn dictation_and_memos() {
  let base = server().await;
  let wav = crate::pcm::wav(&[0i16; 2400], crate::RATE);
  let text = crate::stt::transcribe("k", &base, wav.clone(), "es").await.unwrap();
  assert_eq!(text, format!("heard {} bytes in es", wav.len()));
  let mp3 = crate::tts::speak("k", &base, "Hola", "ara", "es", 1.0).await.unwrap();
  assert_eq!(String::from_utf8(mp3).unwrap(), "mp3:ara:Hola");
}
