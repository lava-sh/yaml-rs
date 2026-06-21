#!/bin/sh
set -e

cd /app

uv pip install --group maturin --system

maturin build --out dist --features mimalloc

uv pip install dist/*.whl --system

python -c "import yaml_rs; print(yaml_rs.__version__)"
