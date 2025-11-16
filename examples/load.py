from pathlib import Path
from pprint import pprint

import yaml_rs


pprint(yaml_rs.load("config.yaml"))

with open("config.yaml", "rb") as F:
    pprint(yaml_rs.load(F.read()))

with Path("config.yaml").open("rb") as config_file:
    pprint(yaml_rs.load(config_file))
