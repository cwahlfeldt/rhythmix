# Contributing to Rhythmix

## Development Setup
1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Install Node.js for frontend: `nvm install --lts`
3. Install dependencies: `cargo build` and `cd visualizer && npm install`

## Running Tests
- Backend: `cargo test`
- Frontend: `cd visualizer && npm test`

## Code Style
- Rust: Follow Rustfmt standards
- JavaScript/React: Follow ESLint rules
- Commit messages: Use conventional commits

## Pull Requests
1. Create a feature branch
2. Write tests for new features
3. Update documentation if needed
4. Submit PR with clear description

## Reporting Issues
- Use GitHub issues template
- Include steps to reproduce
- Provide relevant logs/screenshots