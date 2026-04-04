# Encryption Module

AES-256-GCM encryption for sensitive data storage in FoxNIO.

## Quick Start

```bash
# 1. Generate master key
export FOXNIO_MASTER_KEY=$(openssl rand -base64 32)

# 2. Start application
cargo run
```

## Files

- `encryption.rs` - Core encryption service
- `encryption_global.rs` - Global singleton for encryption service
- `encrypted_field.rs` - SeaORM entity field wrappers

## Usage

```rust
use foxnio::utils::encryption_service;

let enc = encryption_service();

// Encrypt
let encrypted = enc.encrypt("secret")?;

// Decrypt  
let decrypted = enc.decrypt(&encrypted)?;
```

## Key Rotation

```bash
# Set both keys: new_key:old_key
export FOXNIO_MASTER_KEY="new_base64_key:old_base64_key"
```

See [ENCRYPTION.md](../../docs/ENCRYPTION.md) for full documentation.
