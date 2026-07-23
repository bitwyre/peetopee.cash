FROM rust:1.91 AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock* ./
COPY src src
COPY migrations migrations
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/peetopee-api /usr/local/bin/peetopee-api
EXPOSE 8080
CMD ["peetopee-api"]
