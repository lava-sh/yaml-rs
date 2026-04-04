# /// script
# requires-python = ">=3.10"
# dependencies = ["toml-rs == 0.3.7"]
# ///

from pathlib import Path

import toml_rs

toml = Path("Cargo.toml")

load_toml = toml_rs.loads(
    toml.read_text(encoding="utf-8"),
    toml_version="1.1.0",
)

toml_rs.dump(load_toml, toml, toml_version="1.0.0")
