set dotenv-load := true


# Fix linting errors where possible
fix: fmt && check
    cargo clippy --fix --allow-staged --workspace -- -D warnings --no-deps

# Check for linting errors
check:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format the Rust code
[private]
fmt:
    cargo +nightly fmt --all