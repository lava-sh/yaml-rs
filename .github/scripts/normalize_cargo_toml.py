import re
from pathlib import Path


def normalize(text: str) -> str:
    lines = text.splitlines()
    out = []
    in_dependencies = False
    i = 0

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        if re.match(r"^\[dependencies]\s*$", stripped):
            in_dependencies = True
            out.append(line)
            i += 1
            continue

        if in_dependencies and re.match(r"^\[.+]\s*$", stripped):
            in_dependencies = False

        if in_dependencies:
            match = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*\{\s*$", stripped)
            if match:
                dep_name = match.group(1)
                out.append(f"[dependencies.{dep_name}]")
                i += 1
                array_depth = 0
                while i < len(lines):
                    body_line = lines[i].rstrip()
                    body_stripped = body_line.strip()
                    if body_stripped == "}":
                        break
                    if array_depth == 0 and body_stripped.endswith(","):
                        body_line = body_line.rstrip(",")
                    out.append(body_line)
                    array_depth += body_line.count("[") - body_line.count("]")
                    i += 1
                i += 1
                continue

        out.append(line)
        i += 1
    return "\n".join(out) + ("\n" if text.endswith("\n") else "")


path = Path("Cargo.toml")
path.write_text(
    normalize(path.read_text(encoding="utf-8")),
    encoding="utf-8",
)
