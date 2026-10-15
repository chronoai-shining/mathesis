FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl git libgmp10 zstd \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 65532 --create-home --home-dir /opt/verifier verifier \
    && mkdir -p /opt/lean-workspace && chown verifier:verifier /opt/lean-workspace
USER 65532:65532
ENV HOME=/opt/verifier
ENV PATH=/opt/verifier/.elan/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
WORKDIR /opt/lean-workspace
COPY --chown=65532:65532 infra/verifier/lean-toolchain infra/verifier/lakefile.toml ./
RUN curl -fsSL https://github.com/leanprover/elan/releases/download/v4.2.1/elan-x86_64-unknown-linux-gnu.tar.gz -o /tmp/elan.tar.gz \
    && tar -xzf /tmp/elan.tar.gz -C /tmp \
    && /tmp/elan-init -y --no-modify-path --default-toolchain none \
    && elan toolchain install "$(cat lean-toolchain)" \
    && lake update \
    && lake exe cache get \
    && test -f .lake/packages/mathlib/.lake/build/lib/lean/Mathlib.olean \
    && rm /tmp/elan.tar.gz /tmp/elan-init
