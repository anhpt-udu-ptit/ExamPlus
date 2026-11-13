# Cấu hình môi trường phát triển cục bộ cho ExamPlus
# Thiết lập PATH chứa Rust toolchain và GNU MinGW (w64devkit)
$env:PATH = "$env:USERPROFILE\w64devkit\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:LIBRARY_PATH = "$env:USERPROFILE\w64devkit\lib\gcc\x86_64-w64-mingw32\13.2.0;$env:USERPROFILE\w64devkit\x86_64-w64-mingw32\lib"

Write-Host "ExamPlus Environment Activated: Rust + w64devkit MinGW64" -ForegroundColor Green
