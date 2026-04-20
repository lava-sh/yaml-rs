#!/usr/bin/env bash
set -euo pipefail

die() {
  echo "Error: $*" >&2
  exit 1
}

export RUST_HOST
RUST_HOST="$(rustc --print host-tuple)"
shopt -s nullglob
read -r -a interpreters <<<"$INPUTS_INTERPRETER"

python_download_request() {
  local version="$1"
  local os arch libc

  case "$RUNNER_OS" in
    Windows)
      os="windows"
      libc="none"
      case "$INPUTS_PYTHON_ARCH" in
        x64) arch="x86_64" ;;
        x86) arch="x86" ;;
        arm64) arch="aarch64" ;;
        *)
          die "Unsupported Windows python arch: $INPUTS_PYTHON_ARCH"
          ;;
      esac
      ;;
    Linux)
      os="linux"
      libc="gnu"
      case "$INPUTS_TARGET" in
        x86_64) arch="x86_64" ;;
        x86) arch="x86" ;;
        aarch64) arch="aarch64" ;;
        armv7) arch="armv7" ;;
        s390x) arch="s390x" ;;
        ppc64le) arch="powerpc64le" ;;
        riscv64) arch="riscv64" ;;
        *)
          die "Unsupported Linux target: $INPUTS_TARGET"
          ;;
      esac
      ;;
    macOS)
      os="macos"
      libc="none"
      case "$INPUTS_TARGET" in
        x86_64) arch="x86_64" ;;
        aarch64) arch="aarch64" ;;
        universal2) arch="x86_64" ;;
        *)
          die "Unsupported macOS target: $INPUTS_TARGET"
          ;;
      esac
      ;;
    *)
      die "Unsupported runner OS: $RUNNER_OS"
      ;;
  esac

  if [[ "$version" == pypy* ]]; then
    echo "pypy-${version#pypy}-${os}-${arch}-${libc}"
    return
  fi

  if [[ "$version" == *t ]]; then
    echo "cpython-${version%t}+freethreaded-${os}-${arch}-${libc}"
  else
    echo "cpython-${version}-${os}-${arch}-${libc}"
  fi
}

wheel_pattern() {
  local version="$1"
  if [[ "$version" == pypy* ]]; then
    local pypy_minor="${version#pypy}"
    local compact="${pypy_minor/./}"
    echo "initial-wheel/*-pp${compact}-*.whl"
    return
  fi

  local compact="${version//./}"
  if [[ "$version" == *t ]]; then
    compact="${compact%t}"
    echo "initial-wheel/*-cp${compact}-cp${compact}t-*.whl"
  else
    echo "initial-wheel/*-cp${compact}-cp${compact}-*.whl"
  fi
}

resolve_python_path() {
  local request="$1"
  local python_path

  python_path="$(uv python find --no-project "$request" 2> /dev/null || true)"
  if [[ -z "$python_path" ]]; then
    uv python install "$request" || die "Failed to install Python: $request"
    python_path="$(uv python find --no-project "$request")"
  fi
  [[ -n "$python_path" ]] || die "Python not found for request: $request"

  printf '%s\n' "$python_path"
}

venv_python_path() {
  local venv_dir="$1"

  if [[ "$RUNNER_OS" == "Windows" ]]; then
    printf '%s\n' "$venv_dir/Scripts/python.exe"
  else
    printf '%s\n' "$venv_dir/bin/python"
  fi
}

find_matching_wheel() {
  local version="$1"
  local pattern
  local wheels=()
  local wheel

  pattern="$(wheel_pattern "$version")"
  while IFS= read -r wheel; do
    wheels+=("$wheel")
  done < <(compgen -G "$pattern")
  if [[ "${#wheels[@]}" -ne 1 ]]; then
    echo "Expected exactly one wheel for ${version}, found ${#wheels[@]} using pattern: ${pattern}" >&2
    ls -lh initial-wheel >&2
    exit 1
  fi

  printf '%s\n' "${wheels[0]}"
}

install_and_run_wheel() {
  local version="$1"
  local pgo_python="$2"
  local wheel_path

  wheel_path="$(find_matching_wheel "$version")"
  uv pip install --python "$pgo_python" --force-reinstall --no-deps "$wheel_path" || die "Failed to install wheel: $wheel_path"
  "$pgo_python" benchmark/pgo.py
}

setup_python_env() {
  local version="$1"
  local safe_version="${version//./_}"
  local venv_dir=".pgo-venv/${safe_version}"
  local request
  local python_path
  local pgo_python

  request="$(python_download_request "$version")"
  rm -rf "$venv_dir"
  python_path="$(resolve_python_path "$request")"
  uv venv "$venv_dir" --python "$python_path" || die "Failed to create venv: $venv_dir"

  pgo_python="$(venv_python_path "$venv_dir")"
  [[ -f "$pgo_python" ]] || die "Python executable not found: $pgo_python"

  install_and_run_wheel "$version" "$pgo_python"
}

for version in "${interpreters[@]}"; do
  setup_python_env "$version"
done

sysroot="$(rustc --print sysroot)" || die "Failed to get rustc sysroot"
llvm_profdata="$sysroot/lib/rustlib/$RUST_HOST/bin/llvm-profdata"
[[ -f "$llvm_profdata" ]] || die "llvm-profdata not found: $llvm_profdata"
printf 'LLVM_PROFDATA=%s\n' "$llvm_profdata" >> "$GITHUB_ENV"
