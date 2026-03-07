import re
from pathlib import Path


def _strip_comment(line: str) -> str:
    idx = line.find("#")
    return line[:idx] if idx != -1 else line


def normalize(text: str) -> str:
    lines = text.splitlines()
    out: list[str] = []
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
                body_parts: list[str] = []
                i += 1
                while i < len(lines):
                    body_stripped = lines[i].strip()
                    if body_stripped == "}":
                        break
                    part = _strip_comment(lines[i]).strip()
                    if part:
                        body_parts.append(part)
                    i += 1

                inner = " ".join(body_parts)
                inner = re.sub(r"\s+", " ", inner).strip()
                inner = re.sub(r",\s*$", "", inner)
                out.append(f"{dep_name} = {{ {inner} }}")
                i += 1
                continue

        out.append(line)
        i += 1

    return "\n".join(out) + ("\n" if text.endswith("\n") else "")


if __name__ == "__main__":
    path = Path("Cargo.toml")
    path.write_text(
        normalize(path.read_text(encoding="utf-8")),
        encoding="utf-8",
    )
