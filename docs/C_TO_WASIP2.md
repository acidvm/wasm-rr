# Compiling C to WASI Preview 2 Components

## Overview

Compiling C code to WASI Preview 2 (wasip2) components is more complex than compiling to traditional WASI modules. The process involves multiple tools and steps to create a proper WebAssembly component that conforms to the Component Model specification.

## Key Differences: WASI p1 vs WASI p2

- **WASI Preview 1 (wasip1)**: Produces WebAssembly modules (`.wasm` files with version 0x1)
- **WASI Preview 2 (wasip2)**: Produces WebAssembly components (`.wasm` files with version 0x1000d) that follow the Component Model

## Required Tools

1. **WASI SDK** (version 21+ for wasip2 support)
   - Provides `wasm32-wasip2-clang` compiler
   - Download from: https://github.com/WebAssembly/wasi-sdk/releases

2. **wasm-tools**
   - For component manipulation and validation
   - Install: `cargo install wasm-tools`

3. **wit-bindgen** (optional, for WIT-based components)
   - For generating bindings from WIT files
   - Install: `cargo install wit-bindgen-cli`

## Compilation Methods

### Method 1: Direct Compilation with wasm32-wasip2 (WASI SDK 21+)

For simple C programs that only use standard library functions:

```bash
# Compile directly to a wasip2 component
wasm32-wasip2-clang hello.c -o hello.wasm
```

This produces a WASI p2 component directly if your WASI SDK supports it.

### Method 2: Compile to wasip1 and Adapt to wasip2

For older WASI SDK versions or when direct wasip2 compilation isn't available:

```bash
# Step 1: Compile to wasip1 module
wasm32-wasi-clang hello.c -o hello_module.wasm

# Step 2: Download WASI adapter
curl -L -o wasi_snapshot_preview1.command.wasm \
  https://github.com/bytecodealliance/wasmtime/releases/latest/download/wasi_snapshot_preview1.command.wasm

# Step 3: Convert to component using adapter
wasm-tools component new hello_module.wasm \
  --adapt wasi_snapshot_preview1.command.wasm \
  -o hello.wasm
```

### Method 3: Using wit-bindgen for Component Interfaces

For components that need to export/import specific interfaces:

```bash
# Step 1: Generate bindings from WIT
wit-bindgen c ./wit

# Step 2: Implement the generated interface in your C code

# Step 3: Compile with component type
wasm32-wasip2-clang \
  -o my_component.wasm \
  -mexec-model=reactor \
  my_implementation.c \
  generated.c \
  generated_component_type.o
```

## Example: Hello World in C

### hello.c
```c
#include <stdio.h>

int main() {
    printf("Hello, World!\n");
    return 0;
}
```

### Compilation Steps

#### Using WASI SDK 21+ (with wasip2 support):
```bash
# Direct compilation
/path/to/wasi-sdk-21/bin/wasm32-wasip2-clang hello.c -o hello.wasm
```

#### Using older WASI SDK (adapter method):
```bash
# Compile to wasip1
/path/to/wasi-sdk/bin/clang \
  --target=wasm32-wasi \
  --sysroot=/path/to/wasi-sdk/share/wasi-sysroot \
  -o hello_module.wasm \
  hello.c

# Convert to component
wasm-tools component new hello_module.wasm \
  --adapt wasi_snapshot_preview1.command.wasm \
  -o hello.wasm
```

## Verification

To verify you've created a proper WASI p2 component:

```bash
# Check the component structure
wasm-tools component wit hello.wasm

# Validate the component
wasm-tools validate hello.wasm --features component-model

# Check the binary format (should show version 0x1000d for components)
file hello.wasm
```

## Common Issues and Solutions

### Issue: "failed to find export `wasi:cli/run@0.2.0`"
**Solution**: Your component needs the command adapter. Use the `wasi_snapshot_preview1.command.wasm` adapter when converting.

### Issue: "missing _start export"
**Solution**: The module needs to be compiled as a command (with main function) not a reactor. Remove `-mexec-model=reactor` if present.

### Issue: "wasm32-wasip2" target not found
**Solution**: Your WASI SDK version is too old. Download WASI SDK 21 or later which includes wasip2 support.

## Nix Integration

For Nix-based builds, you can fetch the WASI adapter as a fixed-output derivation:

```nix
wasi-adapter = pkgs.fetchurl {
  url = "https://github.com/bytecodealliance/wasmtime/releases/download/v37.0.2/wasi_snapshot_preview1.command.wasm";
  sha256 = "1lazj423za0816xi2sb7lvznrp7is3lkv4pil6nf77yj2v3qjvab";
};
```

Then use it in your build:

```nix
buildPhase = ''
  # Compile C to wasip1 module
  clang --target=wasm32-wasi hello.c -o hello_module.wasm

  # Convert to component
  wasm-tools component new hello_module.wasm \
    --adapt ${wasi-adapter} \
    -o hello.wasm
'';
```

## Current Limitations

1. **Toolchain Integration**: Unlike Rust's `cargo-component`, there's no integrated build system for C/C++ components
2. **Direct Execution**: Running components directly from C/C++ host code is not yet fully supported
3. **WASI SDK Versions**: Not all WASI SDK versions support wasip2 directly
4. **Platform Support**: Some build tools (like wasilibc in Nix) may not be available on all platforms

## Resources

- [WASI SDK Releases](https://github.com/WebAssembly/wasi-sdk/releases)
- [Component Model Documentation](https://component-model.bytecodealliance.org/)
- [wit-bindgen C Support](https://github.com/bytecodealliance/wit-bindgen)
- [WASI Adapters](https://github.com/bytecodealliance/wasmtime/releases)
- [wasm-tools Documentation](https://github.com/bytecodealliance/wasm-tools)