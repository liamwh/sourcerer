# Contributing to Sourcerer

Thank you for considering a contribution!  The following guidelines help keep the project healthy and consistent.

## Development workflow

1. **Fork & clone** the repository.
2. Run the full test-suite:
   ```bash
   cargo test --workspace --all-features
   ```
3. Ensure linting passes:
   ```bash
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```
4. Open a pull-request and fill in the template.

## Commit message conventions

We enforce [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) using the *Angular* style.  CI will fail if your commit messages do not match the pattern.

```
<type>(<scope>): <subject>

<body>

<footer>
```

* **type** – one of `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`.
* **scope** – optional area of the codebase (`store`, `derive`, `docs`, …).
* **subject** – concise description (≤72 char, no trailing period).

Examples:

* `feat(store): add streaming API to EventStore`
* `fix(derive): handle invalid attribute syntax`
* `docs: improve README quick-start`

Automated tooling (`commitlint`) validates these rules (see `.commitlintrc.json`).  If you are using *husky* or similar Git hooks locally, hook into the `commit-msg` phase to get instant feedback.

## Code style

* The workspace is formatted with `rustfmt` (`cargo fmt`).
* Clippy must be clean (`cargo clippy -D warnings`).
* Public items **must** be documented (`#![deny(missing_docs)]`).

---

Feel free to open Issues or Discussions if you have questions or feature ideas!  Happy hacking 🦀