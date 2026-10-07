param([string]$CompilerPath, [string]$ReleasesUrl, [switch]$SkipBuild)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
if (-not $CompilerPath) {
    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    $candidates = @(
        "$root/.clyra/tools/inno/ISCC.exe",
        "${env:ProgramFiles}/Inno Setup 7/ISCC.exe",
        "${env:ProgramFiles(x86)}/Inno Setup 6/ISCC.exe"
    )
    if ($command) { $CompilerPath = $command.Source }
    else { $CompilerPath = $candidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1 }
}
if (-not $CompilerPath -or -not (Test-Path -LiteralPath $CompilerPath)) {
    throw 'Inno Setup is required. Install it or pass -CompilerPath pointing to ISCC.exe.'
}
& "$PSScriptRoot/package-windows.ps1" -ReleasesUrl $ReleasesUrl -SkipBuild:$SkipBuild
$manifestPath = Join-Path $root 'target/package/manifest.json'
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
$version = $manifest.version
$out = Join-Path $root 'target/package'
$stage = Join-Path $out "clyra-$version-windows-x86_64"
& $CompilerPath "/DAppVersion=$version" "/DPackageDir=$stage" "/DOutputDir=$out" "$root/dist/windows/setup.iss"
if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed' }
$installer = Join-Path $out "clyra-$version-windows-x86_64-setup.exe"
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$installer.sha256", "$hash  $([IO.Path]::GetFileName($installer))`n", [Text.UTF8Encoding]::new($false))
$manifest.files | Add-Member -NotePropertyName ([IO.Path]::GetFileName($installer)) -NotePropertyValue @{ sha256 = $hash } -Force
[IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 5), [Text.UTF8Encoding]::new($false))
Write-Host "Installer: $installer"
