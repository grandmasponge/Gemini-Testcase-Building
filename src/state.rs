use std::{env, fs::OpenOptions, io::{BufWriter, Write}, path::Path, time::Duration};

use gemini_client_rs::{types::{Content, ContentPart, FileData, GenerateContentRequest, PartResponse}, GeminiClient};
use base64::{prelude::BASE64_STANDARD, Engine};
use reqwest::{Client, Method};
use serde_json::json;
use tokio::time::sleep;
use crate::{prompt::Prompt, retrive_document, GeminiError, GeminiErrorType};
use serde::{Serialize, Deserialize};

pub struct GeminiState<'a> {
    pub client: GeminiClient,
    pub model_name: &'a str,
    pub history: Vec<Content>,
    api_key: String,
}


impl<'a> GeminiState<'a> {
   pub  fn new(client: GeminiClient, model_name: &'a str, key: String) -> Self {
        Self {
            client,
            model_name,
            api_key: key,
            history: vec![],
        }
    }

    pub async fn prompt(&mut self, filename: &str) -> Result<GeminiResponse, GeminiError> {
        let mut files_uris = vec![];

        let prompt_contents = retrive_document(filename)?;
        let prompt = Prompt::extract(prompt_contents)?;

        for file in prompt.files {
            let file_contents = retrive_document(&file)?;
            let base_contents = BASE64_STANDARD.encode(file_contents);
            let wrapper = self.upload_file(file, base_contents)
            .await?;
            
            files_uris.push(wrapper.file.uri);
        }   


        let mut content_part = vec![ContentPart::Text(prompt.prompt)];
        for uri in files_uris {
            match uri {
                Some(u) => {
                    content_part.push(ContentPart::FileData(FileData{
                        mime_type: "text/plain".to_string(),
                        file_uri: u.clone()
                    }));
                }
                None => {
                    return Err(GeminiError{err_type:GeminiErrorType::MissingFileUri});
                }
            }
        }

        self.history.push(Content { parts: content_part, role: prompt.role });

        
        let res = self.upload_prompt(&self.history).await?;
        self.history.push(Content { parts: vec![ContentPart::Text(res.clone())], role: gemini_client_rs::types::Role::Model });


        Ok(GeminiResponse { filename: filename.to_string(), response: res })

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
        sleep(Duration::from_secs(3)).await;

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

    async fn upload_prompt(&self, content: &Vec<Content>) -> Result<String, GeminiError> {
        let json_content = json!(
            {
                "contents": content,
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
                        return Ok(text.clone());
                    }
                }
            }
        }

        Ok("FAILED TO RECIVE A RESPONSE".to_string())
    }
}



#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct FileWrapper {
    pub file: GeminiFile,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct GeminiFile {
   pub  name: Option<String>,
    #[serde(rename = "displayName")]
    pub  display_name: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: Option<String>,
    #[serde(rename = "createTime")]
    pub create_time: Option<String>,
    #[serde(rename = "updateTime")]
    pub update_time: Option<String>,
    #[serde(rename = "expirationTime")]
    pub expiration_time: Option<String>,
    #[serde(rename = "sha256Hash")]
    pub sha265_hash: Option<String>,
    pub uri: Option<String>,
    #[serde(skip, rename = "downloadUri")]
    pub download_uri: Option<String>,
    #[serde(skip)]
    pub state: Option<String>,
    #[serde(skip)]
    pub source: Option<String>,
    #[serde(skip)]
    pub error: Option<String>,
    #[serde(skip)]
    pub video_meta: Option<String>,
}

pub struct GeminiResponse {
    pub filename: String,
    pub response: String
}


impl GeminiResponse {
    pub fn open_html(&self) {
        let split = self.response.split("```");
        for part in split {
            //check if after the code seperator there is an html part
            let tag = &part[0..3];
            if tag != "html" {
                continue;
            }
            //the tag is there write to a file and display
            let path = Path::new(&self.filename);
            let name =  path.file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.to_string()).unwrap();

            let cwd = env::current_dir().unwrap().to_str().unwrap().to_string();
            
            let file_path = format!("{}/responses/HTML/{}-response-html.html",cwd, name);

            let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file_path).unwrap();

            let mut buf_write = BufWriter::new(file);
            buf_write.write_all(&part[3..].as_bytes()).unwrap();

            webbrowser::open(&format!("file://{}",file_path)).unwrap();
        }
        
    }

    pub fn save_to_file(&self) {
        let path = Path::new(&self.filename);
            let name =  path.file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.to_string()).unwrap();

            let cwd = env::current_dir().unwrap().to_str().unwrap().to_string();
            
            let file_path = format!("{}/responses/AI/{}-response-html.txt",cwd, name);

            let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&file_path).unwrap();

            let mut buf_writer = BufWriter::new(file);

            buf_writer.write_all(self.response.as_bytes()).unwrap();
    }
}