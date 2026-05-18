// Kiro Standard Library: Networking (reqwest)
// Glue layer between Kiro and Rust async HTTP

use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn get(args: Vec<RuntimeVal>) -> HostResult {
    let url = args[0].as_str()?;
    match reqwest::get(url).await {
        Ok(response) => match response.text().await {
            Ok(text) => Ok(RuntimeVal::from(text)),
            Err(_) => Err(KiroError::new("NetworkError")),
        },
        Err(e) => {
            if e.is_builder() {
                Err(KiroError::new("InvalidUrl"))
            } else {
                Err(KiroError::new("NetworkError"))
            }
        }
    }
}

pub async fn post(args: Vec<RuntimeVal>) -> HostResult {
    let url = args[0].as_str()?;
    let body = args[1].as_str()?.to_string();
    let client = reqwest::Client::new();
    match client.post(url).body(body).send().await {
        Ok(response) => match response.text().await {
            Ok(text) => Ok(RuntimeVal::from(text)),
            Err(_) => Err(KiroError::new("NetworkError")),
        },
        Err(e) => {
            if e.is_builder() {
                Err(KiroError::new("InvalidUrl"))
            } else {
                Err(KiroError::new("NetworkError"))
            }
        }
    }
}

pub async fn status(args: Vec<RuntimeVal>) -> HostResult {
    let url = args[0].as_str()?;
    match reqwest::get(url).await {
        Ok(response) => Ok(RuntimeVal::from(response.status().as_u16() as f64)),
        Err(e) => {
            if e.is_builder() {
                Err(KiroError::new("InvalidUrl"))
            } else {
                Err(KiroError::new("NetworkError"))
            }
        }
    }
}

pub async fn body(args: Vec<RuntimeVal>) -> HostResult {
    // Simple passthrough - the response is already a string
    let response = args[0].as_str()?;
    Ok(RuntimeVal::from(response.to_string()))
}
