FROM rustlang/rust:nightly AS chef
RUN cargo install cargo-chef
WORKDIR app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin pithos

FROM debian:buster-slim AS runtime
WORKDIR pithos
COPY --from=builder /app/target/release/pithos /usr/local/bin
COPY --from=builder /app/tls/ ./tls/
ENTRYPOINT ["/usr/local/bin/pithos"]