default: build

# Build the frontend, then the binary that embeds it.
build: frontend
    cargo build --release

# Install frontend dependencies and build frontend/dist.
frontend:
    cd frontend && npm ci && npm run build

test:
    cargo test --workspace
    cd frontend && npm test

lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cd frontend && npm run lint && npm run typecheck

# Backend on :8080 and Vite dev server proxying /api to it.
dev:
    cargo run -p lodestar & cd frontend && npm run dev
