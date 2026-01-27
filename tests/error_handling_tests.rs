use actix_web::ResponseError;
use obsidian_host::error::{AppError, FileSystemError, VaultError};
use std::io;

#[test]
fn test_not_found_error() {
    let err = AppError::NotFound("test.md".to_string());
    let msg = err.user_message();

    assert!(msg.contains("not found"));
    assert!(msg.contains("test.md"));
    assert!(err.recovery_suggestion().is_some());
}

#[test]
fn test_invalid_input_error() {
    let err = AppError::InvalidInput("Invalid file name".to_string());
    let msg = err.user_message();

    assert!(msg.contains("Invalid input"));
}

#[test]
fn test_conflict_error() {
    let err = AppError::Conflict("File modified externally".to_string());
    let msg = err.user_message();

    assert!(msg.contains("conflict"));
    assert!(err.recovery_suggestion().is_some());
}

#[test]
fn test_io_error_context() {
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
    let err = AppError::io_error("reading file", Some("/test/file.md".to_string()), io_err);

    let msg = err.user_message();
    assert!(msg.contains("reading file"));
    assert!(msg.contains("Permission denied"));
    assert!(err.recovery_suggestion().is_some());
}

#[test]
fn test_io_error_not_found() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let err = AppError::io_error("opening file", Some("/missing.md".to_string()), io_err);

    let msg = err.user_message();
    assert!(msg.contains("File or directory not found"));
}

#[test]
fn test_vault_error_already_exists() {
    let err = AppError::VaultError(VaultError::AlreadyExists("/path/to/vault".to_string()));
    let msg = err.user_message();

    assert!(msg.contains("already exists"));
    assert!(msg.contains("/path/to/vault"));
}

#[test]
fn test_vault_error_invalid_path() {
    let err = AppError::VaultError(VaultError::InvalidPath("/invalid/path".to_string()));
    let msg = err.user_message();

    assert!(msg.contains("invalid"));
    assert!(err.recovery_suggestion().is_some());
}

#[test]
fn test_vault_error_not_accessible() {
    let err = AppError::VaultError(VaultError::NotAccessible("/no/access".to_string()));
    let msg = err.user_message();

    assert!(msg.contains("Cannot access"));
    assert!(msg.contains("permissions"));
}

#[test]
fn test_filesystem_error_permission_denied() {
    let err = AppError::FileSystemError(FileSystemError::PermissionDenied {
        path: "/test/file.md".to_string(),
        operation: "writing".to_string(),
    });

    let msg = err.user_message();
    assert!(msg.contains("Permission denied"));
    assert!(msg.contains("writing"));
    assert!(err.recovery_suggestion().is_some());
}

#[test]
fn test_filesystem_error_path_traversal() {
    let err = AppError::FileSystemError(FileSystemError::PathTraversal {
        attempted_path: "../../../etc/passwd".to_string(),
    });

    let msg = err.user_message();
    assert!(msg.contains("security violation"));
}

#[test]
fn test_filesystem_error_file_too_big() {
    let err = AppError::FileSystemError(FileSystemError::FileTooBig {
        path: "huge.md".to_string(),
        size: 100_000_000,
        max_size: 10_000_000,
    });

    let msg = err.user_message();
    assert!(msg.contains("too large"));
    assert!(msg.contains("100000000"));
    assert!(msg.contains("10000000"));
}

#[test]
fn test_filesystem_error_disk_full() {
    let err = AppError::FileSystemError(FileSystemError::DiskFull);
    let msg = err.user_message();

    assert!(msg.contains("disk space"));
    assert!(err.recovery_suggestion().is_some());

    let suggestion = err.recovery_suggestion().unwrap();
    assert!(suggestion.contains("Free up disk space"));
}

#[test]
fn test_filesystem_error_invalid_filename() {
    let err = AppError::FileSystemError(FileSystemError::InvalidFileName {
        name: "file<>name.md".to_string(),
        reason: "contains invalid characters".to_string(),
    });

    let msg = err.user_message();
    assert!(msg.contains("Invalid file name"));
    assert!(msg.contains("invalid characters"));
}

#[test]
fn test_error_display() {
    let err = AppError::NotFound("resource".to_string());
    let display = format!("{}", err);

    assert!(display.contains("not found"));
}

#[test]
fn test_io_error_conversion() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
    let app_err: AppError = io_err.into();

    match app_err {
        AppError::IoError(_) => (),
        _ => panic!("Expected IoError"),
    }
}

#[test]
fn test_recovery_suggestions() {
    // Test that appropriate errors have recovery suggestions
    let not_found = AppError::NotFound("test".to_string());
    assert!(not_found.recovery_suggestion().is_some());

    let conflict = AppError::Conflict("test".to_string());
    assert!(conflict.recovery_suggestion().is_some());

    let disk_full = AppError::FileSystemError(FileSystemError::DiskFull);
    assert!(disk_full.recovery_suggestion().is_some());

    // Test that some errors don't have recovery suggestions
    let internal = AppError::InternalError("test".to_string());
    assert!(internal.recovery_suggestion().is_none());
}

#[test]
fn test_user_friendly_messages() {
    // Verify all error types produce user-friendly messages
    let errors = vec![
        AppError::NotFound("test".to_string()),
        AppError::InvalidInput("test".to_string()),
        AppError::Conflict("test".to_string()),
        AppError::Unauthorized("test".to_string()),
        AppError::Forbidden("test".to_string()),
        AppError::InternalError("test".to_string()),
        AppError::VaultError(VaultError::AlreadyExists("test".to_string())),
        AppError::FileSystemError(FileSystemError::DiskFull),
    ];

    for err in errors {
        let msg = err.user_message();
        // Ensure message is not empty and doesn't contain technical jargon
        assert!(!msg.is_empty());
        assert!(!msg.contains("Error::"));
        assert!(!msg.contains("panic"));
    }
}

#[test]
fn test_error_response_structure() {
    let err = AppError::NotFound("test.md".to_string());
    let _response = err.error_response();

    // Verify response was created successfully
    // In a real integration test, we'd verify the JSON structure
}

#[test]
fn test_helper_constructors() {
    // Test io_error helper
    let io_err = io::Error::new(io::ErrorKind::NotFound, "test");
    let err = AppError::io_error("reading", Some("/test.md".to_string()), io_err);

    match err {
        AppError::IoError(ctx) => {
            assert_eq!(ctx.operation, "reading");
            assert_eq!(ctx.path, Some("/test.md".to_string()));
        }
        _ => panic!("Expected IoError"),
    }
}

#[test]
fn test_all_io_error_kinds() {
    let kinds = vec![
        (io::ErrorKind::NotFound, "File or directory not found"),
        (io::ErrorKind::PermissionDenied, "Permission denied"),
        (io::ErrorKind::AlreadyExists, "File already exists"),
        (io::ErrorKind::InvalidInput, "Invalid input"),
        (io::ErrorKind::TimedOut, "Operation timed out"),
    ];

    for (kind, expected_msg) in kinds {
        let io_err = io::Error::new(kind, "test");
        let err = AppError::io_error("test operation", None, io_err);
        let msg = err.user_message();
        assert!(
            msg.contains(expected_msg),
            "Expected '{}' in '{}'",
            expected_msg,
            msg
        );
    }
}

#[test]
fn test_error_categorization() {
    // Client errors (4xx)
    let client_errors = vec![
        AppError::NotFound("test".to_string()),
        AppError::InvalidInput("test".to_string()),
        AppError::Conflict("test".to_string()),
        AppError::Unauthorized("test".to_string()),
        AppError::Forbidden("test".to_string()),
    ];

    for err in client_errors {
        // All should have user-friendly messages
        assert!(!err.user_message().is_empty());
    }

    // Server errors (5xx)
    let server_errors = vec![AppError::InternalError("test".to_string())];

    for err in server_errors {
        assert!(!err.user_message().is_empty());
    }
}
