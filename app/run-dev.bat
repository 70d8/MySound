@echo off
rem MySound: install packages then start the app in dev mode
cd /d "%~dp0"
echo [MySound] npm install...
call npm install || (echo npm install failed & pause & exit /b 1)
echo [MySound] npm run tauri dev...
call npm run tauri dev
pause
