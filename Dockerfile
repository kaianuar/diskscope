FROM rust:latest
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config build-essential \
    && curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
# deps are fetched at run time by the pipeline (cargo build / npm install)
# using the project's Cargo.lock / package.json.
