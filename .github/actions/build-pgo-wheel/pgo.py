import glob
import logging
import os
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from pprint import pformat

logger = logging.getLogger(__name__)
logging.basicConfig(
    level=logging.INFO,
    format="%(message)s",
)


def run_cmd(*args: str) -> str:
    return subprocess.check_output(args, text=True).strip()


def green(text: str) -> str:
    return f"\x1b[1m\x1b[92m{text}\x1b[0m"


@dataclass(frozen=True)
class Context:
    target: str
    interpreters: list[str]
    workdir: Path
    runner_os: str
    rust_host: str


ctx = Context(
    target=os.environ["INPUTS_TARGET"],
    interpreters=os.environ["INPUTS_INTERPRETER"].split(),
    workdir=Path(os.environ.get("INPUTS_WORKING_DIRECTORY", ".")),
    runner_os=os.environ["RUNNER_OS"],
    rust_host=run_cmd("rustc", "--print", "host-tuple"),
)
logger.info("Context:\n%s", pformat(ctx))


def python_request(version: str) -> str:
    arch = ctx.target.split("-", 1)[0]
    arch = {"i686": "x86", "riscv64gc": "riscv64"}.get(arch, arch)

    match ctx.runner_os:
        case "Linux":
            os_name = "linux"

            if "-musl" in ctx.target:
                libc = "musl"
            elif "-gnu" in ctx.target:
                libc = "gnu"
            else:
                msg = f"Unsupported target {ctx.target}"
                raise RuntimeError(msg)

        case "Windows":
            os_name = "windows"
            libc = "none"

        case "macOS":
            os_name = "macos"
            libc = "none"

            if ctx.target.startswith("universal2"):
                arch = "x86_64"

        case _:
            msg = f"Unsupported OS {ctx.runner_os}"
            raise RuntimeError(msg)

    if version.startswith("pypy"):
        return f"pypy-{version[4:]}-{os_name}-{arch}-{libc}"

    if version.endswith("t"):
        return f"cpython-{version[:-1]}+freethreaded-{os_name}-{arch}-{libc}"

    pattern = f"cpython-{version}-{os_name}-{arch}-{libc}"
    logger.info("%s: %s", green("Python request"), pattern)
    return pattern


def wheel_pattern(version: str) -> str:
    base = ctx.workdir / "initial-wheel"

    if version.startswith("pypy"):
        tag = version[4:].replace(".", "")
        return str(base / f"*-pp{tag}-*.whl")

    tag = version.replace(".", "")

    if version.endswith("t"):
        tag = tag[:-1]
        return str(base / f"*-cp{tag}-cp{tag}t-*.whl")

    pattern = str(base / f"*-cp{tag}-cp{tag}-*.whl")
    logger.info("%s: %s", green("Wheel pattern"), pattern)
    return pattern


def find_wheel(version: str) -> Path:
    wheels = glob.glob(wheel_pattern(version))

    if len(wheels) != 1:
        msg = f"Expected one wheel, got {wheels}"
        raise RuntimeError(msg)

    wheel_path = Path(wheels[0])
    logger.info("%s: %s", green("Found wheel"), wheel_path)
    return wheel_path


def uv_python(request: str) -> Path:
    result = subprocess.run(
        ["uv", "python", "find", "--no-project", request],
        text=True,
        capture_output=True,
        check=False,
    )

    path = result.stdout.strip()

    if not path:
        subprocess.run(["uv", "python", "install", request], check=True)
        path = run_cmd("uv", "python", "find", "--no-project", request)

    return Path(path)


def venv_python(venv: Path) -> Path:
    if ctx.runner_os == "Windows":
        return venv / "Scripts" / "python.exe"

    return venv / "bin" / "python"


def run_profile(version: str) -> None:
    python = uv_python(python_request(version))
    venv = Path(".pgo-venv") / version.replace(".", "_")
    shutil.rmtree(venv, ignore_errors=True)
    subprocess.run(["uv", "venv", str(venv), "--python", str(python)], check=True)
    executable = venv_python(venv)

    subprocess.run(
        [
            "uv",
            "pip",
            "install",
            "--python",
            str(executable),
            "--force-reinstall",
            "--no-deps",
            str(find_wheel(version)),
        ],
        check=True,
    )
    subprocess.run(
        [str(executable), str(ctx.workdir / "benchmark" / "pgo.py")],
        check=True,
    )


for interpreter in ctx.interpreters:
    run_profile(interpreter)

sysroot = Path(run_cmd("rustc", "--print", "sysroot"))

llvm = sysroot / "lib" / "rustlib" / ctx.rust_host / "bin" / "llvm-profdata"

if ctx.runner_os == "Windows":
    llvm = llvm.with_suffix(".exe")

if not llvm.exists():
    msg = f"llvm-profdata not found: {llvm}"
    raise RuntimeError(msg)

logger.info("%s: %s", green("LLVM profdata"), llvm)
logger.info("%s: %s", green("LLVM"), run_cmd(str(llvm), "--version"))


with Path(os.environ["GITHUB_ENV"]).open("a", encoding="utf-8") as f:
    f.write(f"LLVM_PROFDATA={llvm}\n")
