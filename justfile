set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

alias i := install

WHEEL_DIR := "wheel/"

[private]
@default:
    just --list

[windows]
[script("pwsh.exe", "-NoLogo", "-NoProfile", "-Command")]
[doc("Build Python wheel with mimalloc")]
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
