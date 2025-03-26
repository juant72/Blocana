# Coding Standards and Conventions

This document defines the coding standards and conventions to be followed during Blocana development.

## Rust Code Style

### Formatting
- Use `rustfmt` with the default configuration
- Run `cargo fmt` before committing code
- Maximum line length: 100 characters
- Indentation: 4 spaces (no tabs)

### Naming Conventions
- **Types** (structs, enums, traits, type aliases): `PascalCase`
- **Variables** and **functions**: `snake_case`
- **Constants** and **static variables**: `SCREAMING_SNAKE_CASE`
- **Macro names**: `snake_case!`
- **Generic type parameters**: single uppercase letter (`T`, `E`, etc.) or `PascalCase`
- **Lifetimes**: short lowercase (`'a`, `'de`, etc.)

### Documentation
- All public items must have documentation comments (`///`)
- All modules must begin with a module-level documentation comment (`//!`)
- Examples in documentation should be tested when possible
- Follow rustdoc conventions for formatting

Example:
```rust
/// Represents a transaction in the Blocana blockchain.
///
/// # Examples
///
/// ```
/// let tx = Transaction::new(
///     sender,
///     recipient,
///     100,
///     5,
///     0,
///     vec![]
/// );
/// assert!(tx.validate().is_ok());
/// ```
pub struct Transaction {
    // Fields...
}
```

### Code Organization
- Prefer smaller, focused files over large files
- Group related functionality into modules
- Use the following order for items within a file:
  1. Module documentation (`//!`)
  2. Imports (sorted: standard library, external crates, local modules)
  3. Constants/statics
  4. Public types
  5. Private types
  6. Implementations
  7. Tests (in a `#[cfg(test)]` module at the end)

### Error Handling
- Use `Result<T, E>` for operations that can fail
- Define specific error types rather than using generic errors
- Leverage the `?` operator for error propagation
- Avoid unwrap/expect in production code
- Include context in error messages

### Performance Considerations
- Be mindful of heap allocations
- Use `&str` instead of `String` for function parameters when possible
- Consider using `SmallVec` for small collections
- Benchmark performance-critical code
- Optimize only after measuring

### Testing
- All public functionality must have unit tests
- Use integration tests for module interactions
- Write tests for edge cases and error conditions
- Make tests deterministic
- Document test purpose

## Repository Organization

### Branch Structure
- `main`: Always stable, deployable
- `develop`: Integration branch for features
- `feature/X`: Feature branches
- `bugfix/X`: Bug fix branches
- `release/X.Y.Z`: Release preparation branches

### Commit Messages
- Use the imperative mood ("Add feature" not "Added feature")
- Structure: `<type>: <subject>`
- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`
- Keep subject under 50 characters
- Add detailed description when necessary

Example:
```
feat: implement transaction pool

Add a transaction pool that maintains pending transactions
with the following features:
- Prioritization based on fee
- Validation against current state
- Size limitation and expiry
```

### Pull Requests
- Reference related issues
- Include a clear description of changes
- Add tests covering the changes
- Update documentation as needed
- Ensure all CI checks pass
- Request reviews from appropriate team members
- Use PR templates when provided

## Code Quality Tools

### Required Tools
- **rustfmt**: Code formatting
- **clippy**: Linting and static analysis
- **cargo-audit**: Dependency vulnerability scanning
- **cargo-deny**: License compliance checking

### Configuration Files
- `.rustfmt.toml`: Custom formatting rules (if needed)
- `.clippy.toml`: Custom linting rules (if needed)
- `deny.toml`: License and dependency configurations

### CI Checks
All pull requests must pass:
- Build without warnings
- All tests passing
- Rustfmt verification
- Clippy lints with no warnings
- Security audit for vulnerabilities
- License compliance checks

## Documentation Standards

### Project Documentation
- README.md should provide a clear project overview
- CONTRIBUTING.md should explain how to contribute
- Each major feature should have dedicated documentation
- Document architecture decisions in ADRs (Architecture Decision Records)

### API Documentation
- Every public API must be documented
- Include examples for non-trivial functions
- Document panics and errors
- Document performance considerations for critical functions
- Use markdown formatting in doc comments for readability

### Code Comments
- Focus on "why" not "what" (the code shows what it does)
- Add comments for complex algorithms
- Use TODO/FIXME tags for temporary solutions
- Keep comments up-to-date with code changes
- Remove commented-out code; use version control instead

## Versioning and Compatibility

### Semantic Versioning
- Follow [SemVer](https://semver.org/) (MAJOR.MINOR.PATCH)
- Breaking changes increment MAJOR version
- New features increment MINOR version
- Bug fixes increment PATCH version
- Document all changes in CHANGELOG.md

### API Compatibility
- Clearly mark experimental APIs
- Avoid breaking changes when possible
- Deprecate features before removing them
- Provide migration paths for breaking changes
- Use feature flags for backward compatibility when appropriate

### Dependency Management
- Pin dependencies to specific versions in production code
- Regularly update dependencies for security fixes
- Document minimum supported Rust version (MSRV)
- Prefer well-maintained dependencies with compatible licenses

## Security Practices

### Code Security
- Prefer memory-safe abstractions
- Use constant-time comparisons for sensitive data
- Validate all external inputs
- Apply the principle of least privilege
- Follow the OWASP secure coding guidelines

### Sensitive Data Handling
- Never log sensitive information
- Use secure storage for keys and credentials
- Encrypt sensitive data at rest
- Use the `zeroize` crate to clear sensitive memory
- Do not commit secrets to version control

### Dependencies
- Minimize number of dependencies
- Review dependencies before adding them
- Keep dependencies up-to-date
- Prefer well-maintained crates with good security track records
- Check for security vulnerabilities using cargo-audit

## Conclusion

These coding standards ensure consistency, quality, and maintainability across the Blocana codebase. All contributors are expected to adhere to these guidelines. For questions or suggestions regarding these standards, please open an issue on the project repository.

--- End of Document ---
