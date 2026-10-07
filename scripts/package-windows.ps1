param(
    [string]$ReleasesUrl,
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
if ($ReleasesUrl -and -not $ReleasesUrl.StartsWith('https://')) { throw 'Release feed must use HTTPS' }
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    if (-not $SkipBuild) {
        cargo build --release --locked -p clyra
        if ($LASTEXITCODE -ne 0) { throw 'Windows build failed' }
    }
    # Explicit pipes also work for the GUI-subsystem executable in CI. A
    # PowerShell collection match does not populate the scalar $Matches map.
    $probe = [Diagnostics.ProcessStartInfo]::new()
    $probe.FileName = (Resolve-Path -LiteralPath './target/release/clyra.exe').Path
    $probe.Arguments = '--version'
    $probe.UseShellExecute = $false
    $probe.CreateNoWindow = $true
    $probe.RedirectStandardOutput = $true
    $probe.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($probe)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(10000)) {
            $process.Kill()
            throw 'Executable version probe timed out'
        }
        $versionMatch = [regex]::Match($stdout.Result.Trim(), '\Aclyra (\d+\.\d+\.\d+)\z')
        if ($process.ExitCode -ne 0 -or -not $versionMatch.Success) {
            throw "Cannot read executable version: $($stderr.Result)"
        }
        $version = $versionMatch.Groups[1].Value
    } finally { $process.Dispose() }
    $out = Join-Path $root 'target/package'
    $stage = Join-Path $out "clyra-$version-windows-x86_64"
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item -LiteralPath './target/release/clyra.exe' -Destination (Join-Path $stage 'clyra.exe')
    # App-local CRT deployment lets the per-user installer run on machines
    # without Visual Studio or a separately installed VC++ redistributable.
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { throw 'Visual Studio redistributable runtime locator is missing' }
    $vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $vsRoot) { throw 'Visual Studio C++ build tools are required to package their redistributable runtime' }
    $runtime = Get-ChildItem -Path "$vsRoot/VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT/vcruntime140.dll" |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $runtime) { throw 'Visual C++ x64 redistributable runtime is missing' }
    Copy-Item -Path (Join-Path $runtime.DirectoryName '*.dll') -Destination $stage
    $updateConfig = Join-Path $stage 'clyra-update.json'
    if ($ReleasesUrl) {
        [IO.File]::WriteAllText($updateConfig, (@{ releases_url = $ReleasesUrl } | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
    } elseif (Test-Path -LiteralPath $updateConfig) {
        Remove-Item -LiteralPath $updateConfig
    }
    Copy-Item -LiteralPath 'LICENSE','THIRD_PARTY_NOTICES.md' -Destination $stage
    $licenses = Join-Path $stage 'licenses/fonts'
    New-Item -ItemType Directory -Force -Path $licenses | Out-Null
    Copy-Item -Path 'crates/ui/assets/fonts/licenses/*' -Destination $licenses
    Compress-Archive -Path "$stage/*" -DestinationPath "$stage.zip" -Force
    Copy-Item -LiteralPath './target/release/clyra.exe' -Destination "$stage.exe"
    $file = Split-Path "$stage.exe" -Leaf
    $hash = (Get-FileHash -LiteralPath "$stage.exe" -Algorithm SHA256).Hash.ToLowerInvariant()
    $manifest = @{ version = $version; files = @{ $file = @{ sha256 = $hash } } } | ConvertTo-Json -Depth 4
    [IO.File]::WriteAllText((Join-Path $out 'manifest.json'), $manifest, [Text.UTF8Encoding]::new($false))
    Write-Host "Packaged $stage.zip"
} finally { Pop-Location }
