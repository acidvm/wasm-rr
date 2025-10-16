# CLI Tools WASM Compilation Analysis

## Summary

This document analyzes the WASM compilation compatibility of two command-line tools:
- **xh**: HTTP client (v0.25.0) - ❌ Cannot compile to WASM
- **dog**: DNS client (v0.2.0-pre) - ✅ Successfully compiles to WASM

## xh HTTP Client

The xh HTTP client (v0.25.0) cannot be compiled to WebAssembly (WASM) due to incompatible native dependencies, specifically the Oniguruma regex library used by the `syntect` crate for syntax highlighting.

## Compilation Attempt

Target: `wasm32-wasip2`
Date: 2025-10-16
xh Version: 0.25.0

## Build Failure Details

The compilation fails with the following error:

```
error: failed to run custom build command for `onig_sys v69.9.1`

oniguruma/src/regint.h:123:10: fatal error: 'stdlib.h' file not found
```

## Root Cause Analysis

### Primary Blocker: Oniguruma (onig_sys)

The `onig_sys` crate is a native C dependency that provides Oniguruma regex support. It fails to compile for WASM because:

1. **Missing WASI libc support**: The build system cannot locate standard C headers (`stdlib.h`) for the WASM target
2. **Native C compilation**: Oniguruma is written in C and requires platform-specific compilation that isn't compatible with WASM's sandboxed environment

### Dependency Chain

```
xh
└── syntect v5.1 (for syntax highlighting)
    └── onig v6.5.1 (regex engine)
        └── onig_sys v69.9.1 (native C library)
```

## Additional WASM Compatibility Concerns

Beyond the Oniguruma blocker, xh has other dependencies that may pose challenges for WASM:

1. **reqwest**: HTTP client library that relies on system networking
   - WASM networking requires special handling through WASI interfaces
   - Would need significant adaptation for wasi:http support

2. **dirs**: Platform-specific directory access
   - Uses system-specific APIs not available in WASM

3. **rpassword**: Terminal password input
   - Requires direct terminal access not available in WASM

4. **network-interface**: Network interface enumeration
   - Platform-specific networking APIs

5. **Unix socket support**: Feature that's inherently platform-specific

## Potential Solutions

### 1. Replace Oniguruma with Pure Rust Alternative

The `syntect` crate supports an alternative regex backend using `fancy-regex` (pure Rust). This would require:
- Modifying xh's Cargo.toml to use syntect with the `fancy-regex` feature instead of `regex-onig`
- Testing to ensure syntax highlighting still works correctly

### 2. Disable Syntax Highlighting for WASM

Create a WASM-specific build configuration that:
- Removes or stubs out the syntect dependency
- Provides a simplified output without syntax highlighting

### 3. Full WASM Port

A comprehensive port would require:
- Replacing all native dependencies with WASM-compatible alternatives
- Implementing WASI interfaces for networking (wasi:http)
- Removing platform-specific features (Unix sockets, network interfaces)
- Adapting file I/O to use WASI filesystem interfaces

## dog DNS Client

The dog DNS client (v0.2.0-pre) successfully compiles to WebAssembly when TLS features are disabled.

### Successful Compilation

Target: `wasm32-wasip2`
Date: 2025-10-16
dog Version: 0.2.0-pre

### Build Command

```bash
cargo build --target wasm32-wasip2 --no-default-features
```

### Key Success Factors

1. **No Native C Dependencies**: Unlike xh, dog is written in pure Rust without C dependencies
2. **Modular Feature System**: TLS support can be disabled via cargo features
3. **Simple Network Operations**: Basic DNS queries work well with WASI networking

### Runtime Test

The compiled WASM binary runs successfully:

```bash
cargo run -- record examples/dog/target/wasm32-wasip2/debug/dog.wasm -- --help
```

The binary displays help information and executes correctly in the WASM runtime.

### Limitations in WASM

When compiled for WASM, dog has the following limitations:
- No TLS support (DNS-over-TLS disabled)
- No HTTPS support (DNS-over-HTTPS disabled)
- Network operations limited to WASI capabilities
- No platform-specific network interface enumeration

## Conclusion

This analysis demonstrates the varying levels of WASM compatibility among CLI tools:

### xh (HTTP Client)
While xh is an excellent HTTP client for native platforms, its current architecture relies heavily on native system dependencies that make direct WASM compilation impossible. A WASM port would require significant refactoring to replace native dependencies with WASM-compatible alternatives, particularly the Oniguruma regex engine used for syntax highlighting.

### dog (DNS Client)
Dog successfully compiles to WASM when non-essential features are disabled. This demonstrates that well-architected Rust applications with modular feature systems can be adapted for WASM environments. The dog WASM binary provides core DNS query functionality, making it a viable tool for WASM-based environments that need DNS resolution capabilities.

### Key Takeaways

1. **Pure Rust is WASM-Friendly**: Applications written in pure Rust without C dependencies have a much better chance of WASM compilation
2. **Feature Flags Matter**: Modular architecture with cargo features allows disabling incompatible components
3. **Network Operations Are Possible**: Basic networking through WASI interfaces works for simple protocols like DNS
4. **Trade-offs Are Necessary**: WASM compatibility often requires sacrificing platform-specific features