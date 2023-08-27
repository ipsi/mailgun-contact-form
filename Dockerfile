FROM rust:bullseye as builder

WORKDIR /usr/src/app
COPY . .
# Will build and cache the binary and dependent crates in release mode
RUN cargo build --release && mv ./target/release/mailgun-contact-form ./mailgun-contact-form

# Runtime image
FROM debian:bullseye-slim

RUN apt update; \
    apt install -y --no-install-recommends \
        ca-certificates

# Run as "app" user
RUN useradd -ms /bin/bash app

USER app
WORKDIR /app

# Get compiled binaries from builder's cargo install directory
COPY --from=builder /usr/src/app/mailgun-contact-form /app/mailgun-contact-form

# Run the app
CMD ./mailgun-contact-form