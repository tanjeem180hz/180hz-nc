$winlibsBin = 'C:\Users\pbpb4\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin'
$winlibsTargetBin = 'C:\Users\pbpb4\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\x86_64-w64-mingw32\bin'
$cargoBin = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\bin"
$env:PATH = "$cargoBin;$winlibsBin;$winlibsTargetBin;$env:PATH"

& cargo $args
exit $LASTEXITCODE
