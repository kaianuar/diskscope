FROM rust:latest
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config build-essential  \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# deps are fetched at run time by the pipeline (cargo build) using the project's Cargo.lock
