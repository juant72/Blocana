# Developer Workflow Guide

This document outlines the recommended developer workflow for contributing to Blocana.

## Development Environment Setup

### Prerequisites
- Rust 1.70+ (we recommend using rustup)
- Git
- A code editor with Rust support (we recommend VS Code with rust-analyzer)
- (Optional) Docker for containerized testing

### Initial Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/encrypia/blocana.git
   cd blocana
   ```

2. **Install Rust components**:
   ```bash
   rustup component add rustfmt clippy
   ```

3. **Install recommended VS Code extensions**:
   - rust-analyzer
   - Better TOML
   - Even Better TOML
   - CodeLLDB (for debugging)
   - GitLens (for improved Git integration)

4. **Set up pre-commit hooks**:
   ```bash
   cp scripts/pre-commit .git/hooks/pre-commit
   chmod +x .git/hooks/pre-commit
   ```

## Development Workflow

### 1. Prepare Your Task

1. **Select an issue** from the issue tracker or create a new issue
2. **Comment on the issue** to let others know you're working on it
3. **Understand requirements** by reviewing related documentation

### 2. Create a Feature Branch

```bash
git checkout develop
git pull
git checkout -b feature/your-feature-name
```

### 3. Develop Your Changes

1. **Write code** following our [Coding Standards](./coding-standards.md)
2. **Write tests** for your implementation
3. **Run local tests** frequently:
   ```bash
   cargo test
   ```
4. **Format your code**:
   ```bash
   cargo fmt
   ```
5. **Check for lint errors**:
   ```bash
   cargo clippy
   ```
6. **Commit changes** with meaningful commit messages:
   ```bash
   git add .
   git commit -m "feat: add transaction validation"
   ```

### 4. Keep Your Branch Updated

```bash
git fetch origin
git rebase origin/develop
```

### 5. Submit Your Changes

1. **Push your branch**:
   ```bash
   git push -u origin feature/your-feature-name
   ```
2. **Create a Pull Request** against the `develop` branch
3. **Fill out the PR template** with details about your changes
4. **Request code review** from appropriate team members
5. **Address review feedback** by making additional commits or amending existing ones

### 6. Finalize Your Contribution

1. **Ensure CI passes** on your PR
2. **Update documentation** if necessary
3. **Squash commits** if requested by reviewers
4. **Wait for approval and merge**

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific tests
cargo test transaction::tests::test_validation

# Run tests with logging
RUST_LOG=debug cargo test -- --nocapture
```

### Debugging

1. **Enable Debug Logging**:
   ```rust
   env_logger::init();
   debug!("Variable value: {:?}", variable);
   ```

2. **Use VS Code Debugging**:
   - Set breakpoints in the editor
   - Use the "Run and Debug" panel (Ctrl+Shift+D)
   - Select "Cargo Test" configuration

### Performance Testing

```bash
# Run benchmarks (requires nightly Rust)
cargo +nightly bench

# Profile execution with flamegraph
cargo flamegraph --bin blocana
```

## Common Issues and Solutions

### "Cargo.lock is out of date"
```bash
cargo update
```

### Error when building after pulling changes
```bash
cargo clean && cargo build
```

### Test hangs or won't finish
Check for deadlocks, especially in concurrent code or when using locks or mutexes.

## Resources

- [Rust Documentation](https://doc.rust-lang.org/book/)
- [Blocana Technical Specifications](../specifications/stage1-technical-specs.md)
- [SledDB Documentation](https://docs.rs/sled)
- [Project Issue Tracker](https://github.com/encrypia/blocana/issues)

--- End of Document ---
