from pathlib import Path
from pprint import pprint

import yaml_rs

pprint(yaml_rs.load("config.yaml"))

pprint(yaml_rs.load(Path("config.yaml").read_bytes()))

with Path("config.yaml").open("rb") as config_file:
    pprint(yaml_rs.load(config_file))
