#!/bin/bash

set -euo pipefail

VERSION="${1:-}"

IMAGE="ghcr.io/astral-sh/uv:python3.14-alpine"

run_prebuilt() {
  docker run --rm "$IMAGE" sh -c '
    uv pip install yaml-rs==0.0.9 --system
    python -c "import yaml_rs; print(yaml_rs.__version__)"
  ' 2>&1 | tee /dev/stderr | grep -q "initial-exec TLS resolves to dynamic definition"
}

run_build_from_source() {
  docker run --rm \
    -v /tmp/__build__:/parser \
    -w /parser \
    "$IMAGE" sh -c '
      set -e

      apk add --no-cache \
        curl \
        build-base \
        musl-dev \
        openssl-dev

      curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly
      . "$HOME/.cargo/env"

      rustc --version

      uv pip install "maturin[patchelf]" -U --system

      maturin build \
        --release \
        --out /parser/dist \
        --features mimalloc

      uv pip install /parser/dist/*.whl --system

      python -c "import yaml_rs; print(yaml_rs.__version__)"
    '
}

main() {
  if [[ "$VERSION" == "0.2.1" ]]; then
    run_prebuilt
  else
    echo "Building from source..."
  fi
}

main "$@"