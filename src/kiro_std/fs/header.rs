// Kiro Standard Library: File System (tokio::fs)
// Glue layer between Kiro and Rust async file operations

use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn read(args: Vec<RuntimeVal>) -> HostResult {
    let path = args[0].as_str()?;
    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(RuntimeVal::from(content)),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Err(KiroError::new("NotFound"))
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                Err(KiroError::new("PermissionDenied"))
            } else {
                Err(KiroError::new("IoError"))
            }
        }
    }
}

pub async fn write(args: Vec<RuntimeVal>) -> HostResult {
    let path = args[0].as_str()?;
    let content = args[1].as_str()?;
    match tokio::fs::write(path, content).await {
        Ok(()) => Ok(RuntimeVal::Void),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                Err(KiroError::new("PermissionDenied"))
            } else {
                Err(KiroError::new("IoError"))
            }
        }
    }
}

pub async fn exists(args: Vec<RuntimeVal>) -> HostResult {
    let path = args[0].as_str()?;
    match tokio::fs::try_exists(path).await {
        Ok(exists) => Ok(RuntimeVal::from(exists)),
        Err(_) => Ok(RuntimeVal::from(false)),
    }
}

pub async fn remove(args: Vec<RuntimeVal>) -> HostResult {
    let path = args[0].as_str()?;
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(RuntimeVal::Void),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Err(KiroError::new("NotFound"))
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                Err(KiroError::new("PermissionDenied"))
            } else {
                Err(KiroError::new("IoError"))
            }
        }
    }
}

pub async fn list(args: Vec<RuntimeVal>) -> HostResult {
    let path = args[0].as_str()?;
    match tokio::fs::read_dir(path).await {
        Ok(mut entries) => {
            let mut names = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(RuntimeVal::from(name.to_string()));
                }
            }
            Ok(RuntimeVal::List(names))
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                Err(KiroError::new("NotFound"))
            } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                Err(KiroError::new("PermissionDenied"))
            } else {
                Err(KiroError::new("IoError"))
            }
        }
    }
}
