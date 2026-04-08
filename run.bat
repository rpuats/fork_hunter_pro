@echo off
echo ========================================
echo    Ghost Imperium v2.0 (Rust Edition)
echo ========================================
echo.
echo Запуск сервера...
echo.
echo Web UI: http://localhost:3000
echo API:    http://localhost:3000/api/v1/
echo WS:     ws://localhost:3000/ws
echo.
echo Нажмите Ctrl+C для остановки
echo.

cd /d "%~dp0"
cargo run --release --bin fork_hunter_bin
