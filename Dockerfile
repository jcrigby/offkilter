# Builds the web app (with the wasm kernel) and the document server into
# one small image. Run with a volume on /data to keep documents.
#
#   docker build -t offkilter .
#   docker run -p 8080:8080 -v offkilter-data:/data offkilter

FROM rust:1-bookworm AS kernel
RUN rustup target add wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version 0.2.128 --locked
WORKDIR /src
COPY . .
RUN cargo build -p ok-wasm --target wasm32-unknown-unknown --release \
    && wasm-bindgen --target web --out-dir apps/web/src/wasm --out-name ok_wasm \
       target/wasm32-unknown-unknown/release/ok_wasm.wasm \
    && cargo build -p ok-server --release

FROM node:22-bookworm-slim AS web
WORKDIR /src/apps/web
COPY apps/web/package.json apps/web/package-lock.json ./
RUN npm ci
COPY apps/web ./
COPY --from=kernel /src/apps/web/src/wasm ./src/wasm
RUN npm run build

FROM debian:bookworm-slim
RUN useradd -r -u 1000 offkilter && mkdir -p /data && chown offkilter /data
COPY --from=kernel /src/target/release/ok-server /usr/local/bin/ok-server
COPY --from=web /src/apps/web/dist /srv/web
USER offkilter
EXPOSE 8080
VOLUME ["/data"]
CMD ["ok-server", "--static", "/srv/web", "--data", "/data", "--port", "8080"]
