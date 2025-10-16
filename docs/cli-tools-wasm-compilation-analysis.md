# CLI Tools WASM Compilation Analysis

## Summary

This document analyzes the WASM compilation compatibility of two command-line tools:
- **xh**: HTTP client (v0.25.0) - ❌ Cannot compile to WASM
- **dog**: DNS client (v0.2.0-pre) - ❌ Cannot compile to WASM

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

The dog DNS client (v0.2.0-pre) cannot be compiled to WebAssembly due to transitive dependencies on OpenSSL.

### Compilation Attempt

Target: `wasm32-wasip2`
Date: 2025-10-16
dog Version: 0.2.0-pre (commit 721440b)

### Build Failure Details

Even with all features disabled (`--no-default-features`), the compilation fails with:

```
error: Could not find directory of OpenSSL installation
openssl-sys = 0.9.61
```

### Root Cause Analysis

While dog itself appears to be pure Rust, its dependency tree includes crates that have transitive dependencies on OpenSSL:

1. **native-tls**: Even when optional, its presence in the dependency tree can cause issues
2. **Transitive dependencies**: Some dependencies in the tree require OpenSSL regardless of feature flags
3. **Build system assumptions**: The build process assumes availability of system libraries

### Attempted Workarounds

1. Disabled all default features: `buildNoDefaultFeatures = true`
2. Explicitly set empty features: `buildFeatures = []`
3. Attempted to build with only IDNA support

None of these approaches resolved the OpenSSL dependency issue.

## Conclusion

This analysis demonstrates the challenges of compiling existing CLI tools to WebAssembly:

### xh (HTTP Client)
The xh HTTP client cannot compile to WASM due to its direct dependency on Oniguruma, a C-based regex library used by the syntect crate for syntax highlighting. This is a clear case where native C dependencies block WASM compilation.

### dog (DNS Client)
Despite appearing to be pure Rust with optional TLS features, dog cannot compile to WASM due to transitive dependencies on OpenSSL. Even with all features disabled, the dependency tree still includes crates that require OpenSSL at build time. This highlights a more subtle compatibility issue where transitive dependencies can prevent WASM compilation.

### Key Takeaways

1. **Direct C Dependencies Are Deal-Breakers**: Tools with direct C dependencies (like xh's Oniguruma) cannot compile to WASM without replacing those dependencies
2. **Transitive Dependencies Matter**: Even "pure Rust" projects can fail to compile if their dependencies have hidden native requirements
3. **Feature Flags Aren't Always Enough**: Disabling features may not remove all problematic dependencies from the build graph
4. **WASM-First Design Required**: Tools need to be designed with WASM in mind from the start, carefully vetting all dependencies
5. **Existing Tools Need Major Refactoring**: Most existing CLI tools would require significant restructuring to become WASM-compatible