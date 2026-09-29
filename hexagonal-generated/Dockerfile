FROM rust:1-bookworm AS chef
WORKDIR /app

# Toolchain pinned by rust-toolchain.toml.
COPY rust-toolchain.toml ./
RUN rustup toolchain install && cargo install cargo-chef --locked

# The recipe lists every manifest and nothing else, so the dependency
# layer below survives until a Cargo.toml changes.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --locked -p hexagonal-generated --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked -p hexagonal-generated

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin app
COPY --from=builder /app/target/release/hexagonal-generated /usr/local/bin/hexagonal-generated
RUN mkdir /data && chown app /data
VOLUME /data
ENV DATABASE_URL=sqlite:///data/hexagonal-generated.db?mode=rwc
USER app
ENV HEXAGONAL_GENERATED_ADDR=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/hexagonal-generated"]
