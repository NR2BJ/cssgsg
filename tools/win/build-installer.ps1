# 윈도우 설치기를 만든다 → build\installer\cssgsg-setup.exe
#
#   powershell -ExecutionPolicy Bypass -File tools\win\build-installer.ps1 0.1.0
#
# - 입력기 DLL과 엔진 호스트를 릴리스로 빌드한다(따로: 같이 빌드하면 코어 기능이 합쳐져 호스트의 Mozc 코드가 DLL에도 들어간다).
# - 설정 앱은 build-settings.ps1이 빌드한다(.NET 10 SDK, MSVC 링커).
# - Mozc 엔진은 build\mozc-out에 있어야 한다: tools\mozc\build-windows.ps1로 빌드했거나 워크플로(mozc-windows.yml) 묶음을 푼 것.
# - Inno Setup 6의 ISCC가 필요하다: 환경 변수 ISCC, PATH, 또는 보통 설치 자리(Program Files (x86), %LOCALAPPDATA%\Programs).
#   winget install JRSoftware.InnoSetup. GitHub windows 러너에는 들어 있다.
# - 버전에 -가 붙으면(0.2.0-beta.1) 파일 버전에는 앞 숫자만 쓴다.
# - 이 파일은 UTF-8(BOM)이다.
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Version = $Version.TrimStart('v')
if ($Version -notmatch '^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$') { throw "not a version: $Version (e.g. 0.1.0, 0.2.0-beta.1)" }
$Numeric = ($Version -split '-')[0]

function Find-Iscc {
    if ($env:ISCC -and (Test-Path $env:ISCC)) { return $env:ISCC }
    $onPath = Get-Command iscc -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    foreach ($dir in "${env:ProgramFiles(x86)}\Inno Setup 6", "$env:ProgramFiles\Inno Setup 6", "$env:LOCALAPPDATA\Programs\Inno Setup 6") {
        $candidate = Join-Path $dir 'ISCC.exe'
        if (Test-Path $candidate) { return $candidate }
    }
    throw 'ISCC.exe (Inno Setup 6) not found: winget install JRSoftware.InnoSetup'
}
$Iscc = Find-Iscc

foreach ($f in 'build\mozc-out\lib\cssgsg_mozc.dll', 'build\mozc-out\data\mozc.data', 'build\mozc-out\MOZC_VERSION') {
    if (-not (Test-Path (Join-Path $Root $f))) { throw "missing $f (tools\mozc\build-windows.ps1 or the mozc-windows.yml package)" }
}

# 설정 앱(WinUI 3, .NET 10 SDK가 필요하다) → build\settings\publish
& (Join-Path $PSScriptRoot 'build-settings.ps1') $Version

Push-Location $Root
# cargo는 진행 상황을 표준 오류로 낸다. Windows PowerShell 5.1은 그걸 오류로 바꾸니 잠깐 푼다.
$ErrorActionPreference = 'Continue'
try {
    foreach ($package in 'cssgsg-tip', 'cssgsg-host') {
        cargo build --release -p $package 2>&1 | ForEach-Object { "$_" }
        if ($LASTEXITCODE -ne 0) { throw "cargo build -p $package failed" }
    }
    & $Iscc /Q "/DVersion=$Version" "/DNumericVersion=$Numeric" "/DRepo=$Root" (Join-Path $Root 'win\installer\cssgsg.iss')
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed (exit code $LASTEXITCODE)" }
}
finally {
    Pop-Location
    $ErrorActionPreference = 'Stop'
}
$Setup = Join-Path $Root 'build\installer\cssgsg-setup.exe'
Write-Host ("cssgsg {0} -> {1} ({2:N1} MB)" -f $Version, $Setup, ((Get-Item $Setup).Length / 1MB))
