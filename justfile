set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

alias i := install
alias b := bump-python-dependencies
alias l := lint

WHEEL_DIR := "wheels/"

[unix]
_activate_venv := "source .venv/bin/activate"

[windows]
_activate_venv := '.\.venv\Scripts\Activate.ps1'

[private]
@default:
    just --list

[doc("Build Python wheel with mimalloc")]
[script("pwsh.exe", "-NoLogo", "-NoProfile", "-Command")]
[windows]
install:
    $ErrorActionPreference = "Stop"

    if (Test-Path {{ WHEEL_DIR }}) {
        Remove-Item {{ WHEEL_DIR }} -Recurse -Force
    }

    {{ _activate_venv }}

    maturin build --out {{ WHEEL_DIR }} --release --features mimalloc

    if (Get-Command uv -ErrorAction SilentlyContinue) {
        Write-Host "uv found, using uv"
        uv pip install toml-rs --no-index --find-links {{ WHEEL_DIR }} --force-reinstall
    } else {
        Write-Host "uv not found, using pip"
        pip install toml-rs --no-index --find-links {{ WHEEL_DIR }} --force-reinstall
    }

[doc("Bump Python dependencies")]
[script("pwsh.exe", "-NoLogo", "-NoProfile", "-Command")]
[windows]
bump-python-dependencies:
    $ErrorActionPreference = "Stop"

    $branch = git branch --show-current

    if (-not $branch.StartsWith("bump")) {
        $n = 1

        while ($true) {
            $newBranch = "bump-$n"
            git show-ref --verify --quiet "refs/heads/$newBranch"
            if ($LASTEXITCODE -ne 0) {
                break
            }
            $n++
        }

        git switch -c $newBranch
        Write-Host "Switched to $newBranch"
    }

    uv run scripts/bump_python_deps.py
    git add pyproject.toml

    git diff --cached --quiet
    if ($LASTEXITCODE -eq 0) {
        Write-Host "skipping commit"
    } else {
        git commit -m "bump Python dependencies"
    }

[doc("Run all lints")]
lint:
    {{ _activate_venv }}
    # Python
    -ruff check
    -ty check
    # Markdown
    -rumdl check
    # Spell Check
    -typos
    # GitHub Actions
    -zizmor .github/
    # Rust
    -cargo fmt-nightly --check
    -cargo clippy --all-features --all-targets