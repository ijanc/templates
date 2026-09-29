FROM rust:1-bookworm AS builder
WORKDIR /app

# Toolchain pinned by rust-toolchain.toml.
COPY rust-toolchain.toml ./
RUN rustup toolchain install

# Dependencies first so they are cached until Cargo.toml changes.
COPY Cargo.toml Cargo.lock build.rs ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && touch src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY . .
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /nonexistent --shell /usr/sbin/nologin app
COPY --from=builder /app/target/release/{{project-name}} /usr/local/bin/{{project-name}}
{%- if store == "sqlite" %}
RUN mkdir /data && chown app /data
VOLUME /data
ENV DATABASE_URL=sqlite:///data/{{project-name}}.db?mode=rwc
{%- endif %}
USER app
ENV {{env_prefix}}_ADDR=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://127.0.0.1:8080/healthz || exit 1
ENTRYPOINT ["/usr/local/bin/{{project-name}}"]
