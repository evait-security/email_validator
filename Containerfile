# ── Wolfi Container Image ──────────────────────────────────────────
# Build:
#   docker build -t email_validator:latest -f Containerfile .
#   docker build -t email_validator:latest -f Containerfile --build-arg ARCH=arm64 .
#
# Or use the GitHub Actions workflow (release.yml) to auto-build on tag.

ARG ARCH=x86_64

FROM cgr.dev/chainguard/wolfi-base:latest

# Copy the statically-linked musl binary
COPY target/${ARCH}-unknown-linux-musl/release/email_validator /usr/local/bin/email_validator

# Security: run as non-root
USER nonroot

EXPOSE 8080

# Default: API server on 0.0.0.0:8080 (overridable via $BIND_ADDR)
ENTRYPOINT ["email_validator", "api"]