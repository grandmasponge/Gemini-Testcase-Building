use gemini_client_rs::types::Role;
use crate::{GeminiError, GeminiErrorType};


pub enum RoleG {
    User,
    System,
    Model,
    Tool,
}

impl RoleG {
    fn transform(self) -> Role {
        match self {
            Self::User => Role::User,
            Self::Model => Role::Model,
            Self::System => Role::System,
            Self::Tool => Role::Tool,
        }
    }
}

pub struct Prompt {
    pub role: gemini_client_rs::types::Role,
    pub prompt: String,
    pub files: Vec<String>,
}

impl From<&str> for RoleG {
    fn from(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "model" => RoleG::Model,
            "user" => RoleG::User,
            "system" => RoleG::System,
            "tool" => RoleG::Tool,
            _ => RoleG::User,
        }
    }
}

impl Prompt {
    pub fn extract(contents: String) -> Result<Self, GeminiError> {
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
                    role = Some(RoleG::from(content).transform());
                }
                "prompt" => {
                    prompt = Some(content.to_string());
                }
                "files" => {
                    let prompt_files = content.split(',');
                    for file in prompt_files {
                        files.push(file.to_string());
                    }
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

