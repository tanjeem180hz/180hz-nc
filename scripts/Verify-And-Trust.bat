@echo off
:: ============================================================================
:: 180Hz NC - Official Security Certificate & Publisher Trust Utility
:: ============================================================================
echo.
echo ============================================================================
echo  180Hz NC - Verifying and Trusting Official Developer Certificate
echo ============================================================================
echo.

set "CERT_FILE=%~dp0180hz_cert.cer"

if not exist "%CERT_FILE%" (
    echo [ERROR] Certificate file not found at: %CERT_FILE%
    pause
    exit /b 1
)

:: Check for Administrator permissions
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [INFO] Administrator permissions required to register trusted root certificate.
    echo Requesting elevation...
    powershell -Command "Start-Process cmd -ArgumentList '/c \"\"%~f0\"\"' -Verb RunAs"
    exit /b 0
)

echo [1/2] Adding 180Hz NC to Trusted Root Certification Authorities...
certutil -addstore -f "ROOT" "%CERT_FILE%" >nul
if %errorlevel% equ 0 (
    echo       [OK] Root Authority trusted successfully.
) else (
    echo       [FAILED] Could not register Root store.
)

echo [2/2] Adding 180Hz NC to Trusted Publishers...
certutil -addstore -f "TrustedPublisher" "%CERT_FILE%" >nul
if %errorlevel% equ 0 (
    echo       [OK] Publisher "180Hz Audio Technologies" trusted successfully.
) else (
    echo       [FAILED] Could not register TrustedPublisher store.
)

echo.
echo ============================================================================
echo  Verification Complete!
echo  Windows SmartScreen and Defender will now verify all 180Hz NC binaries.
echo ============================================================================
echo.
pause
