# ==============================================================================
# ExamPlus - Trình Tạo Phím Tắt Ngoài Màn Hình Desktop (Create Desktop Shortcut)
# ==============================================================================

$ErrorActionPreference = "Stop"

# Xác định thư mục ứng dụng
$AppDir = $PSScriptRoot
$ExePath = Join-Path $AppDir "ExamPlus.exe"

# Kiểm tra file ExamPlus.exe có tồn tại không
if (-not (Test-Path $ExePath)) {
    # Nếu đang chạy từ thư mục gốc hoặc thư mục scripts
    $ParentExe = Join-Path (Split-Path $AppDir -Parent) "ExamPlus.exe"
    if (Test-Path $ParentExe) {
        $AppDir = Split-Path $AppDir -Parent
        $ExePath = $ParentExe
    } else {
        Write-Host "LỖI: Không tìm thấy tệp ExamPlus.exe trong cùng thư mục!" -ForegroundColor Red
        Write-Host "Vui lòng đặt tệp script này cùng thư mục với ExamPlus.exe." -ForegroundColor Yellow
        Pause
        exit 1
    }
}

try {
    # Lấy đường dẫn màn hình Desktop của người dùng
    $DesktopPath = [Environment]::GetFolderPath("Desktop")
    $ShortcutPath = Join-Path $DesktopPath "ExamPlus.lnk"

    # Sử dụng Windows Script Host Shell COM object
    $WshShell = New-Object -ComObject WScript.Shell
    $Shortcut = $WshShell.CreateShortcut($ShortcutPath)
    $Shortcut.TargetPath = $ExePath
    $Shortcut.WorkingDirectory = $AppDir
    $Shortcut.Description = "ExamPlus - Hệ Thống Thi Trắc Nghiệm GDPT 2018"
    $Shortcut.IconLocation = "$ExePath,0"
    $Shortcut.Save()

    Write-Host ""
    Write-Host "==================================================================" -ForegroundColor Green
    Write-Host " [THANH CONG] Da tao loi tat ExamPlus tren Man hinh Desktop!" -ForegroundColor Green
    Write-Host "==================================================================" -ForegroundColor Green
    Write-Host "Duong dan phím tat: $ShortcutPath" -ForegroundColor Cyan
    Write-Host "Tro toi file:        $ExePath" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "Giao vien gio day chi can click dup vao bieu tuong ExamPlus tren" -ForegroundColor White
    Write-Host "man hinh de mo ngay phan mem ma khong can thao tac nao khac." -ForegroundColor White
    Write-Host "==================================================================" -ForegroundColor Green
    Write-Host ""
} catch {
    Write-Host "LỖI: Không thể tạo phím tắt: $_" -ForegroundColor Red
}

Start-Sleep -Seconds 2
