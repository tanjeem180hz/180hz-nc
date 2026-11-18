$winlibsBin = 'C:\Users\pbpb4\AppData\Local\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin'
$cargoBin = "$env:USERPROFILE\.cargo\bin"
$env:PATH = "$cargoBin;$winlibsBin;$env:PATH"

& cargo $args
exit $LASTEXITCODE
