# Download cloudflared.exe for Windows x64
$ErrorActionPreference = "Stop"

$toolsDir = Join-Path $PSScriptRoot "..\tools"
if (-not (Test-Path $toolsDir)) {
    New-Item -ItemType Directory -Path $toolsDir | Out-Null
}

$destPath = Join-Path $toolsDir "cloudflared.exe"
if (Test-Path $destPath) {
    $len = (Get-Item $destPath).Length
    if ($len -gt 1000000) {
        Write-Host "cloudflared.exe already exists: $destPath ($len bytes)"
        exit 0
    }
}

Write-Host "Querying GitHub release for cloudflared..."
$headers = @{ "User-Agent" = "ExamPlus-Downloader" }
$release = Invoke-RestMethod -Uri "https://api.github.com/repos/cloudflare/cloudflared/releases/latest" -Headers $headers
$asset = $release.assets | Where-Object { $_.name -eq "cloudflared-windows-amd64.exe" }

if (-not $asset) {
    Write-Error "Asset cloudflared-windows-amd64.exe not found in release"
    exit 1
}

Write-Host "Downloading $($asset.name) from $($asset.browser_download_url)..."
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $destPath -Headers $headers -UseBasicParsing

$finalLen = (Get-Item $destPath).Length
Write-Host "Downloaded successfully: $destPath ($finalLen bytes)"
