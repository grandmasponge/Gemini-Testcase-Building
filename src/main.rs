use std::{env::VarError, fmt::Display, fs::OpenOptions, io::Read};

use dotenv::dotenv;
use gemini_client_rs::{
    GeminiClient,
    types::{Content, ContentPart, GenerateContentRequest, PartResponse, Role},
};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::json;

struct Prompt {
    role: gemini_client_rs::types::Role,
    prompt: String,
    files: Vec<String>,
}

#[warn(non_camel_case_types)]
pub enum Role_G {
    User,
    System,
    Model,
    Tool,
}

impl Role_G {
    fn transform(self) -> Role {
        match self {
            Self::User => Role::User,
            Self::Model => Role::Model,
            Self::System => Role::System,
            Self::Tool => Role::Tool,
        }
    }
}

struct GeminiState<'a> {
    client: GeminiClient,
    model_name: &'a str,
    api_key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct FileWrapper {
    file: GeminiFile,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
struct GeminiFile {
    name: Option<String>,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(rename = "sizeBytes")]
    size_bytes: Option<String>,
    #[serde(rename = "createTime")]
    create_time: Option<String>,
    #[serde(rename = "updateTime")]
    update_time: Option<String>,
    #[serde(rename = "expirationTime")]
    expirationTime: Option<String>,
    #[serde(rename = "sha256Hash")]
    sha265_hash: Option<String>,
    uri: Option<String>,
    #[serde(skip, rename = "downloadUri")]
    download_uri: Option<String>,
    #[serde(skip)]
    state: Option<String>,
    #[serde(skip)]
    source: Option<String>,
    #[serde(skip)]
    error: Option<String>,
    #[serde(skip)]
    video_meta: Option<String>,
}

#[derive(Debug, Clone)]
enum GeminiErrorType {
    FailedRequest,
    PromptGenerationError,
    MissingRoleError,
    MissingPromptError,
    InvalidHeader,
    MissingHeader,
}

#[derive(Debug, Clone)]
struct GeminiError {
    err_type: GeminiErrorType,
}

impl Display for GeminiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Gemini failed")
    }
}

impl From<&str> for Role_G {
    fn from(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "model" => Role_G::Model,
            "user" => Role_G::User,
            "system" => Role_G::System,
            "tool" => Role_G::Tool,
            _ => Role_G::User,
        }
    }
}

impl<'a> GeminiState<'a> {
    fn new(client: GeminiClient, model_name: &'a str, key: String) -> Self {
        Self {
            client,
            model_name,
            api_key: key,
        }
    }

    pub async fn upload_file(
        &self,
        filename: String,
        contents: String,
    ) -> Result<FileWrapper, GeminiError> {
        let client = Client::new();
        let url = format!(
            "https://generativelanguage.googleapis.com/upload/v1beta/files?key={}",
            self.api_key
        );

        let json_body = json!({
            "file": { "display_name": filename, "mimeType" : "text/plain" }
        });
        

        let res = client
            .request(Method::POST, url)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header(
                "X-Goog-Upload-Header-Content-Length",
                contents.len().to_string(),
            )
            .header("X-Goog-Upload-Header-Content-Type", "text/plain")
            .header("Content-Type", "application/json")
            .body(json_body.to_string())
            .send()
            .await
            .map_err(|_| GeminiError {
                err_type: GeminiErrorType::FailedRequest,
            })?;
        println!("{res:?}");
       
        let upload_url = match res.headers().get("x-goog-upload-url") {
            Some(url) => url.to_str().unwrap().to_string(),
            None => return Err(GeminiError { err_type: GeminiErrorType::MissingHeader })
        };

        println!("upload_url: {upload_url}");

        let res = match client.request(Method::POST, upload_url)
        .header("Content-Length", contents.len().to_string())
        .header("X-Goog-Upload-Offset", "0")
        .header("X-Goog-Upload-Command", "upload, finalize")
        .body(contents)
        .send()
        .await
        {
            Ok(res) => res,
            Err(_e) => return Err(GeminiError{err_type:GeminiErrorType::FailedRequest}),
        };
       
        let res_text = res.text()
        .await
        .unwrap();

        let val: FileWrapper = serde_json::from_str(&res_text).unwrap();

        Ok(val)
    }

    async fn prompt(&self, content: Vec<Content>) -> Result<String, GeminiError> {
        let json_content = json!(
            {
                "contents":content,
            }
        );

        let request: GenerateContentRequest = match serde_json::from_value(json_content) {
            Ok(content) => content,
            Err(e) => {
                println!("{}", e);
                return Err(GeminiError {
                    err_type: GeminiErrorType::FailedRequest,
                });
            }
        };

        let response = match self
            .client
            .generate_content(self.model_name, &request)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                println!("{}", e);
                return Err(GeminiError {
                    err_type: GeminiErrorType::FailedRequest,
                });
            }
        };

        if let Some(candidates) = response.candidates {
            for candidate in &candidates {
                for content in &candidate.content.parts {
                    if let PartResponse::Text(text) = content {
                        println!("{}", text);
                    }
                }
            }
        }

        Ok(String::new())
    }
}

impl Prompt {
    fn extract(contents: String) -> Result<Self, GeminiError> {
        let iter = contents.split('|');
        let mut role = None;
        let mut prompt = None;
        let mut files = Vec::new();

        for part in iter {
            let mut type_split = part.splitn(2, ':');

            let label = type_split.next().ok_or(GeminiError {
                err_type: GeminiErrorType::PromptGenerationError,
            })?;

            let content = type_split.next().ok_or(GeminiError {
                err_type: GeminiErrorType::PromptGenerationError,
            })?;

            match label.to_lowercase().as_str() {
                "role" => {
                    role = Some(Role_G::from(content).transform());
                }
                "prompt" => {
                    prompt = Some(content.to_string());
                }
                "files" => {
                    files = content.split(',').map(|s| s.trim().to_string()).collect();
                }
                _ => {
                    return Err(GeminiError {
                        err_type: GeminiErrorType::PromptGenerationError,
                    });
                }
            }
        }

        Ok(Self {
            role: role.ok_or(GeminiError {
                err_type: GeminiErrorType::MissingRoleError,
            })?,
            prompt: prompt.ok_or(GeminiError {
                err_type: GeminiErrorType::MissingPromptError,
            })?,
            files,
        })
    }
}

fn retrive_document(filename: &str) -> Result<String, ()> {
    let file_res = OpenOptions::new().read(true).create(false).open(filename);
    let mut file = match file_res {
        Ok(f) => f,
        Err(_e) => {
            println!(
                "Failed open file at path: `{}` please make sure the file is at expected location",
                filename
            );
            return Err(());
        }
    };

    let mut buffer = String::new();
    match file.read_to_string(&mut buffer) {
        Ok(bytes) => {
            if bytes > 0 {
                println!("read: {} bytes from file", bytes);
                Ok(buffer)
            } else {
                println!("file is empty");
                Err(())
            }
        }
        Err(_) => {
            println!("failed to read bytes from file please make sure the file is not corrupted");
            Err(())
        }
    }
}

fn get_api_key() -> Result<String, VarError> {
    dotenv().ok();
    std::env::var("API_KEY")
}

#[tokio::main]
async fn main() {
    let api_key: String = get_api_key().expect("failed to retrive API-KEY");

    let client = GeminiClient::new(api_key.clone());
    let model_name = "gemini-2.0-flash";

    let state = GeminiState::new(client, model_name, api_key);
    
    let uri = state.upload_file("test-fil1".to_string(), "test meow meow test meow".to_string()).await.unwrap();

    let _str = state
        .prompt(vec![Content {
            parts: vec![ContentPart::Text(String::from(
                "extract the text from the followign test-fil1 document",
            ))],
            role: Role::User,
        }])
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use gemini_client_rs::GeminiClient;
    use crate::{get_api_key, retrive_document, GeminiState};



    #[test]
    fn file_test_prompt() {
        let filename = "./prompt/test-prompt.txt";
        let buffer = retrive_document(filename).unwrap();

        assert_eq!(
            buffer,
            "Role:User|Prompt:how mutch wood could a wood chuck chuck if a wood chuck could chuck wood?\n"
        );
    }

    #[test]
    fn file_test_template() {
        let filename = "./templates/test_1_template.html";
        let buffer = retrive_document(filename).unwrap();

        let html_identifier = &buffer[0..15];

        assert_eq!(html_identifier, "<!DOCTYPE html>");
    }
}
