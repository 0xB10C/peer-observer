param (
    [string]$ImageName
)

if (-not $ImageName) {
    Write-Host "Usage: .\run.ps1 -ImageName <docker-image-name>"
    exit 1
}

docker run $ImageName
