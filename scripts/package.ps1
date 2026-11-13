# ==============================================================================
# ExamPlus - Kịch Bản Đóng Gói Bản Phát Hành (Release Packaging Script)
# ==============================================================================
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"

if (Test-Path (Join-Path $PSScriptRoot "Cargo.toml")) {
    $RootDir = $PSScriptRoot
} else {
    $RootDir = Join-Path $PSScriptRoot ".."
}
Set-Location $RootDir

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "             ExamPlus - Đóng Gói Bản Phát Hành v1.0.0            " -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan

# 1. Biên dịch Release Binary nếu chưa có switch -SkipBuild
if (-not $SkipBuild) {
    Write-Host "[1/4] Dang bien dich ExamPlus o che do Release..." -ForegroundColor Yellow
    . .\env.ps1
    cargo build --release --bin examplus-app
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Bien dich Release that bai!"
        exit 1
    }
}

$ReleaseBin = Join-Path $RootDir "target\release\examplus-app.exe"
if (-not (Test-Path $ReleaseBin)) {
    Write-Error "Khong tim thay tap tin $ReleaseBin!"
    exit 1
}

# 2. Tạo cấu trúc thư mục phát hành
Write-Host "[2/4] Chuan bi thu muc phat hanh Portable..." -ForegroundColor Yellow
$DistRoot = Join-Path $RootDir "dist"
$PortableDir = Join-Path $DistRoot "ExamPlus"

# Dừng các tiến trình đang khóa tệp trong dist nếu có
$distProcs = Get-Process -Name "cloudflared", "ExamPlus", "examplus-app" -ErrorAction SilentlyContinue | Where-Object {
    $_.Path -and ($_.Path.StartsWith($DistRoot, [System.StringComparison]::OrdinalIgnoreCase))
}
if ($distProcs) {
    $distProcs | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
}

if (-not (Test-Path $PortableDir)) {
    New-Item -ItemType Directory -Force -Path $PortableDir | Out-Null
}

# 3. Sao chép các tệp thực thi và tiện ích
Write-Host "[3/4] Sao chep ExamPlus.exe, cloudflared.exe va scripts..." -ForegroundColor Yellow

# Copy ExamPlus.exe
Copy-Item -Force $ReleaseBin (Join-Path $PortableDir "ExamPlus.exe")

# Copy cloudflared.exe
$CloudflaredSrc = Join-Path $RootDir "tools\cloudflared.exe"
if (Test-Path $CloudflaredSrc) {
    Copy-Item -Force $CloudflaredSrc (Join-Path $PortableDir "cloudflared.exe")
} else {
    Write-Warning "Khong tim thay tools/cloudflared.exe. Tunnel se can tai khi su dung."
}

# Copy / Create Start-ExamPlus.bat
$BatSrc = Join-Path $RootDir "scripts\Start-ExamPlus.bat"
if (Test-Path $BatSrc) {
    Copy-Item $BatSrc (Join-Path $PortableDir "Start-ExamPlus.bat")
} else {
    $BatContent = "@echo off`r`nsetlocal`r`ncd /d `"`%~dp0`"`r`nstart `"`" `"`%~dp0ExamPlus.exe`"`r`n"
    Set-Content -Path (Join-Path $PortableDir "Start-ExamPlus.bat") -Value $BatContent -Encoding ASCII
}

# Copy Create-Desktop-Shortcut.ps1
$ShortcutSrc = Join-Path $RootDir "scripts\Create-Desktop-Shortcut.ps1"
Copy-Item $ShortcutSrc (Join-Path $PortableDir "Create-Desktop-Shortcut.ps1")

# Tạo file Hướng dẫn sử dụng tiếng Việt
$ReadmeContent = @"
================================================================================
                    EXAMPLUS - HỆ THỐNG THI TRẮC NGHIỆM GDPT 2018
                            PHIÊN BẢN PORTABLE WINDOWS
================================================================================

1. HƯỚNG DẪN KHỞI CHẠY (DÀNH CHO GIÁO VIÊN):
--------------------------------------------------------------------------------
- Cách 1 (Mở nhanh): Click đúp chuột vào file "ExamPlus.exe" hoặc "Start-ExamPlus.bat" để mở giao diện
  phần mềm. Ứng dụng sẽ tự động mở cửa sổ đồ họa mà KHÔNG cần mở trình duyệt
  và KHÔNG hiện cửa sổ đen dòng lệnh (terminal).

- Cách 2 (Tạo lối tắt ra màn hình): Click chuột phải vào file
  "Create-Desktop-Shortcut.ps1" -> Chọn "Run with PowerShell".
  Biểu tượng ExamPlus sẽ được tạo ngay trên màn hình Desktop của máy tính.

2. CÁCH SỬ DỤNG CHÍNH:
--------------------------------------------------------------------------------
- Bước 1: Mở app ExamPlus -> Vào tab "Quản Lý Đề Thi".
- Bước 2: Bấm "Nạp Đề Word / PDF" hoặc chọn file từ Google Drive. Phần mềm sẽ tự động nhận diện
  câu hỏi, công thức Toán (MathType SVG / MathML) và bảng biến thiên/hình vẽ (hiển thị xem trước ngay trong app).
- Bước 3: Chọn số lượng mã đề muốn xáo (ví dụ: 4 mã đề) -> Bấm "Tạo Mã Đề Xáo & Mở Phòng".
- Bước 4: Thí sinh dùng điện thoại hoặc máy tính truy cập địa chỉ hiển thị trên app
  (ví dụ: http://192.168.1.15:8080) để vào làm bài.
  * Nếu muốn thí sinh thi từ xa qua 4G/Internet, bấm nút "Bật Cloudflare Tunnel".
- Bước 5: Khi thí sinh nộp bài xong, giáo viên bấm "Xuất Excel" để tải file .xlsx
  bảng điểm tổng hợp và nhật ký cảnh báo vi phạm.

3. TÍNH NĂNG PORTABLE (CẮM USB DÙNG NGAY):
--------------------------------------------------------------------------------
- Thư mục này chứa đầy đủ:
  + ExamPlus.exe: Ứng dụng thi độc lập.
  + cloudflared.exe: Cầu nối mạng an toàn trực tiếp từ Cloudflare.
  + examplus.db: Cơ sở dữ liệu SQLite cục bộ (tự tạo khi chạy).
- Giáo viên có thể copy toàn bộ thư mục này vào USB và cắm sang bất kỳ máy tính
  Windows 10 / Windows 11 nào là sử dụng được ngay lập tức, không cần cài đặt.
================================================================================
"@
Set-Content -Path (Join-Path $PortableDir "README_HUONG_DAN.txt") -Value $ReadmeContent -Encoding UTF8

# 4. Tạo gói ZIP nén lưu trữ (chỉ đóng gói tệp phát hành sạch, không kèm CSDL cục bộ)
Write-Host "[4/4] Tao file nen ZIP phat hanh..." -ForegroundColor Yellow
$ZipPath = Join-Path $DistRoot "ExamPlus-v1.0.0-Windows-x64-Portable.zip"
if (Test-Path $ZipPath) {
    Remove-Item -Force $ZipPath
}
$DistItems = Get-ChildItem -Path $PortableDir -File | Where-Object {
    $_.Name -in @("ExamPlus.exe", "cloudflared.exe", "Start-ExamPlus.bat", "Create-Desktop-Shortcut.ps1", "README_HUONG_DAN.txt")
} | ForEach-Object { $_.FullName }
Compress-Archive -Path $DistItems -DestinationPath $ZipPath

Write-Host ""
Write-Host "==================================================================" -ForegroundColor Green
Write-Host " [THANH CONG] DA HOAN TAT DONG GOI CA 2 DANG PHAT HANH!" -ForegroundColor Green
Write-Host "==================================================================" -ForegroundColor Green
Write-Host "1. Dang Thu Muc Portable: $PortableDir" -ForegroundColor Cyan
Write-Host "   (Mang theo tren USB, click dup ExamPlus.exe la chay ngay)" -ForegroundColor Gray
Write-Host "2. Dang File Nen ZIP:     $ZipPath" -ForegroundColor Cyan
Write-Host "   (San sang tai len Google Drive / Gui qua Zalo, Email)" -ForegroundColor Gray
Write-Host "==================================================================" -ForegroundColor Green
