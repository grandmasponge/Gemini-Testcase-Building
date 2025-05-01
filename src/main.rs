mod prompt;
mod state;
use std::{env::VarError, fmt::Display, fs::OpenOptions, io::Read, os::macos::raw::stat, time::Duration};
use dotenv::dotenv;
use gemini_client_rs::GeminiClient;
use state::{GeminiResponse, GeminiState};
use tokio::time::sleep;



#[derive(Debug, Clone)]
enum GeminiErrorType {
    FailedRequest,
    PromptGenerationError,
    MissingRoleError,
    MissingPromptError,
    InvalidHeader,
    MissingHeader,
    FileReadError,
    MissingFileUri,
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

fn retrive_document(filename: &str) -> Result<String, GeminiError> {
    let file_res = OpenOptions::new().read(true).create(false).open(filename);
    let mut file = match file_res {
        Ok(f) => f,
        Err(_e) => {
            println!(
                "Failed open file at path: `{}` please make sure the file is at expected location",
                filename
            );
            return Err(GeminiError { err_type: GeminiErrorType::FileReadError} );
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
                return Err(GeminiError { err_type: GeminiErrorType::FileReadError} );
            }
        }
        Err(_) => {
            println!("failed to read bytes from file please make sure the file is not corrupted");
            return Err(GeminiError { err_type: GeminiErrorType::FileReadError} );
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

    let mut state = GeminiState::new(client, model_name, api_key);
    
    //prompt 1 Inital prompt
    state.prompt("./prompt/inital-prompt.txt").await
    .unwrap()
    .save_to_file();
    //sleep to prevent overloading
    sleep(Duration::from_secs(5)).await;
    //prompt 2 Require generation
    let res = state.prompt("./prompt/gen-requirements.txt")
    .await
    .unwrap();
    res.save_to_file();
    //prompt 3 Generate test
    let res = state.prompt("./prompt/gen-test-cases.txt").
    await
    .unwrap();
    res.save_to_file();
    //prompt 4 Generate test case description

    //prompt 5 Explain test case properties

    //prompt 6 Generate coverage report

    //prompt 7 Generate traceability matrix

    //prompt 8 Generate Gap report
    
}