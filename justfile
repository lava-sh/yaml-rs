set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

alias i := install
alias b := bump-dependency-groups

WHEEL_DIR := "wheel/"

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

    .\.venv\Scripts\Activate.ps1

    maturin build --out {{ WHEEL_DIR }} --release --features mimalloc

    $wheel = Get-ChildItem {{ WHEEL_DIR }}/*.whl | Select-Object -First 1

    if (Get-Command uv -ErrorAction SilentlyContinue) {
        Write-Host "uv found, using uv"
        uv pip install $wheel.FullName --force-reinstall
    } else {
        Write-Host "uv not found, using pip"
        pip install $wheel.FullName --force-reinstall
    }

[doc("Bump Python dependency-groups")]
[script("pwsh.exe", "-NoLogo", "-NoProfile", "-Command")]
[windows]
bump-dependency-groups:
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
        git commit -m "bump python dependency-groups"
    }
