# Go to WASI Preview 2 Component Guide

This guide explains how to compile Go programs to WASI Preview 2 (WASIp2) components in the wasm-rr repository.

## Overview

Go programs can be compiled to WASIp2 components using **TinyGo**, which is currently the only Go compiler that supports the WASIp2 target. The standard Go compiler only supports WASIp1 and does not have WASIp2 or Component Model support.

## Requirements

- TinyGo 0.33.0 or later (WASIp2 support added in v0.33.0, August 2024)
- wasm-tools (for validating components)
- Nix development environment (provided by this repository)

## Quick Start

### 1. Write Your Go Program

Create a simple Go program:

```go
package main

import "fmt"

func main() {
    fmt.Println("Hello, World from Go!")
}
```

### 2. Build with Nix

The repository's Nix flake provides a build environment with TinyGo:

```bash
# Build the go_hello_world example
nix build .#go_hello_world-wasm

# Build all examples including Go
nix build .
```

### 3. Manual Compilation (Outside Nix)

If you have TinyGo installed locally:

```bash
# Compile to WASIp2 component
tinygo build -target=wasip2 -o output.wasm main.go

# Verify it's a proper component (should show version 0x1000d)
wasm-tools validate output.wasm --features component-model
```

## Build Configuration in flake.nix

The repository uses this Nix derivation for building Go components:

```nix
go_hello_world-wasm = pkgs.stdenv.mkDerivation {
  name = "go_hello_world-wasm";
  src = ./examples/go_hello_world;

  nativeBuildInputs = with pkgs; [
    tinygo
    wasm-tools
  ];

  buildPhase = ''
    # Set up HOME directory for TinyGo
    export HOME=$TMPDIR

    # Compile Go to WASIp2 component using TinyGo
    tinygo build -target=wasip2 -o go_hello_world.wasm main.go
  '';

  installPhase = ''
    mkdir -p $out
    cp go_hello_world.wasm $out/
  '';
};
```

## Testing with wasm-rr

Once built, test your Go component with wasm-rr:

```bash
# Record execution
cargo run -- record result/go_hello_world.wasm -t trace.json

# Replay execution
cargo run -- replay result/go_hello_world.wasm trace.json
```

## TinyGo Optimization Flags

TinyGo provides several optimization levels:

```bash
# Minimal optimizations
tinygo build -target=wasip2 -opt=0 -o output.wasm main.go

# Few optimization passes
tinygo build -target=wasip2 -opt=1 -o output.wasm main.go

# Most optimizations for performance
tinygo build -target=wasip2 -opt=2 -o output.wasm main.go

# Balance performance and code size
tinygo build -target=wasip2 -opt=s -o output.wasm main.go

# Aggressive code size reduction (default)
tinygo build -target=wasip2 -opt=z -o output.wasm main.go
```

## Limitations

### TinyGo Language Support

TinyGo has incomplete Go language support compared to the standard compiler:

- **No `recover()` support on WebAssembly** - Panic recovery doesn't work
- **Partial reflection support** - Most reflection works, some parts unsupported
- **Cgo partially supported** - Limited C interop capabilities
- **Maps functional but potentially slower** - Different implementation than standard Go

### Standard Library Limitations

Many standard library packages don't compile or work fully:

- Some stdlib functions are stubbed or unimplemented
- Networking support in WASIp2 is still pending
- Known issue: Output may be truncated after 4096 bytes (TinyGo issue #5012)

### Binary Size Comparison

| Compiler | Binary Size | WASIp2 Support |
|----------|-------------|----------------|
| TinyGo | <100KB possible | ✅ Yes |
| Standard Go | 2MB+ minimum | ❌ No |

## Using wit-bindgen for Custom Interfaces

For components with custom WIT interfaces:

1. Install wit-bindgen-go:
```bash
go install github.com/bytecodealliance/go-modules/cmd/wit-bindgen-go@latest
```

2. Generate Go bindings from WIT:
```bash
wit-bindgen-go generate ./wit
```

3. Implement the generated interfaces in your Go code

4. Compile with TinyGo:
```bash
tinygo build -target=wasip2 -o component.wasm main.go
```

## Comparison: Go vs C for WASIp2

| Aspect | Go (TinyGo) | C (Zig) |
|--------|-------------|---------|
| **Toolchain** | TinyGo 0.33.0+ | Zig + wasm-tools |
| **WASIp2 Support** | Native | Via adapter |
| **Binary Size** | ~700KB (Hello World) | ~20KB (Hello World) |
| **Language Features** | Partial Go support | Full C support |
| **Memory Management** | Garbage Collection | Manual |
| **Build Complexity** | Simple (one step) | Two steps (compile + adapt) |
| **Standard Library** | Partial | Full libc |

## Example: Adding a New Go Component

1. Create the example directory:
```bash
mkdir -p examples/my_go_app
```

2. Write your Go program:
```go
// examples/my_go_app/main.go
package main

import (
    "fmt"
    "os"
)

func main() {
    args := os.Args[1:]
    fmt.Printf("Arguments: %v\n", args)
}
```

3. Add to flake.nix (following the pattern of go_hello_world-wasm)

4. Build and test:
```bash
nix build .#my_go_app-wasm
cargo run -- record result/my_go_app.wasm -t trace.json -- arg1 arg2
```

## Troubleshooting

### "mkdir /homeless-shelter: read-only file system"

TinyGo needs a HOME directory. In Nix builds, set:
```nix
export HOME=$TMPDIR
```

### Component validation fails

Ensure you're using TinyGo 0.33.0+ with `-target=wasip2`. Earlier versions or wrong target will produce WASIp1 modules.

### Program panics without recovery

TinyGo doesn't support `recover()` on WebAssembly. Design your error handling without relying on panic recovery.

### Output truncated

Known TinyGo issue (#5012) - output may be truncated after 4096 bytes. For large outputs, consider chunking or using file I/O.

## Resources

- [TinyGo Documentation](https://tinygo.org/docs/)
- [TinyGo WASIp2 Release Notes](https://github.com/tinygo-org/tinygo/releases/tag/v0.33.0)
- [Component Model Go Support](https://github.com/bytecodealliance/go-modules)
- [WASI Preview 2 Specification](https://github.com/WebAssembly/WASI/blob/main/preview2/README.md)

## Summary

TinyGo provides the only current path for compiling Go to WASIp2 components. While it has limitations compared to standard Go, it produces small, efficient components suitable for many use cases. The integration with wasm-rr allows for deterministic recording and replay of Go components alongside Rust and C examples.