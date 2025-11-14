from pathlib import Path
from pprint import pprint

import yaml_rs

with Path("config.yaml").open("rb") as config_file:
    pprint(yaml_rs.load(config_file))
