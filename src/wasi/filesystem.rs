// Filesystem interception module for recording file operations
// Currently just records operations without making them deterministic
//
// Note: Since the filesystem trait implementations in wasmtime have complex
// signatures with TrappableError and other types, we're not directly intercepting
// at the trait level. Instead, we provide helper methods that could be used
// by custom implementations in the future.