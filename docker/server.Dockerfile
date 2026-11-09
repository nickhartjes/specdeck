# Sharing service image (signalling and TURN credentials), with the web viewer built in (web-viewer
# design decision 5). Build from the repository root:
#   docker build -f docker/server.Dockerfile -t specdeck-server .
# On a network that only allows a company npm registry, hand the build your npm config as a secret;
# it is mounted for the npm steps only and never stored in the image:
#   docker build --secret id=npmrc,src=$HOME/.npmrc -f docker/server.Dockerfile .

# The web viewer's WebAssembly package (crates/web/pkg), with the wasm-bindgen CLI at the exact
# version crates/web pins, as `just web-wasm` does.
FROM rust:1.99-slim-trixie AS wasm
RUN rustup target add wasm32-unknown-unknown \
 && cargo install wasm-bindgen-cli --version 0.2.130 --locked
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p specdeck-web --target wasm32-unknown-unknown \
 && wasm-bindgen --target web --out-dir crates/web/pkg target/wasm32-unknown-unknown/release/specdeck_web.wasm

# The web viewer's page (app/ui/dist-web).
FROM node:26-trixie-slim AS ui
RUN --mount=type=secret,id=npmrc,target=/root/.npmrc,required=false \
    npm install --global pnpm@12.16.1
WORKDIR /src
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY app/ui/package.json app/ui/
RUN --mount=type=secret,id=npmrc,target=/root/.npmrc,required=false \
    pnpm install --frozen-lockfile --filter @specdeck/ui
COPY app/ui app/ui
# The page states the release it is built from: the workspace version.
COPY Cargo.toml ./
COPY --from=wasm /src/crates/web/pkg crates/web/pkg
RUN pnpm --dir app/ui build:web

# The service, with the page embedded.
FROM rust:1.99-slim-trixie AS build
WORKDIR /src
COPY . .
COPY --from=ui /src/app/ui/dist-web app/ui/dist-web
RUN cargo build --release --locked -p specdeck-server --features web-viewer

FROM debian:trixie-slim
RUN useradd --system --uid 10001 specdeck
COPY --from=build /src/target/release/specdeck-server /usr/local/bin/specdeck-server
# Numeric, so Kubernetes can check runAsNonRoot.
USER 10001
EXPOSE 8080
ENV SPECDECK_LISTEN=0.0.0.0:8080
ENTRYPOINT ["specdeck-server"]
