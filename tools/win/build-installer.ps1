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
# - 설치기를 만들기 전에 실어 보낼 프로그램이 시스템 DLL과 같이 싣는 DLL만 쓰는지 dumpbin으로 확인한다(Assert-Dependencies).
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

# 실어 보낼 프로그램이 쓰는 DLL(dumpbin /dependents, 지연 로드 포함)이 모두 다음 중 하나인지 확인한다. 맥 0.7.2 설정 앱이
# 빌드 폴더의 dylib를 가리켜 다른 맥에서 뜨지 않았던 것과 같은 일(빌드한 PC에서만 도는 설치본)을 막는다.
#   - API set(api-ms-win-*, ext-ms-win-*): 윈도우가 이름으로 푼다.
#   - 같은 폴더에 같이 설치하는 것(설정 앱 폴더의 Windows App SDK DLL, cssgsg_config.dll 등).
#   - System32에 있고 "Microsoft Windows"로 서명된 윈도우 구성 요소. VC++ 런타임(vcruntime·msvcp 등)은 빌드 PC에 깔려 있어도
#     다른 PC에는 없을 수 있어 안 된다: 우리 것은 C 런타임을 정적으로 링크하거나 윈도우에 든 UCRT를 쓴다.
# 실행 중에 이름·경로로 읽는 DLL(설정 앱의 cssgsg_config.dll, 호스트의 cssgsg_mozc.dll)은 import에 없어서, 파일이 제자리에
# 있는지는 build-settings.ps1과 아래 Mozc 확인이 본다. 그 DLL들이 쓰는 DLL은 여기서 본다.
function Find-Dumpbin {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $found = & $vswhere -latest -products * -find 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe' | Select-Object -First 1
        if ($found) { return $found }
    }
    $onPath = Get-Command dumpbin -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    throw 'dumpbin.exe (MSVC) not found'
}

$SystemDlls = @{}
function Test-SystemDll([string]$Name) {
    $key = $Name.ToLowerInvariant()
    if (-not $SystemDlls.ContainsKey($key)) {
        $path = Join-Path $env:SystemRoot "System32\$Name"
        $ok = $false
        if ($key -notmatch '^(vcruntime|msvcp|msvcr|concrt|vccorlib|vcomp|mfc|ucrtbased)' -and (Test-Path -LiteralPath $path -PathType Leaf)) {
            $signature = Get-AuthenticodeSignature -LiteralPath $path
            $ok = $signature.Status -eq 'Valid' -and $signature.SignerCertificate.Subject -match '^CN=Microsoft Windows,'
        }
        $SystemDlls[$key] = $ok
    }
    return $SystemDlls[$key]
}

# $Files: 한 폴더에 같이 설치하는 프로그램들, $Shipped: 그 폴더에 같이 설치하는 파일 이름들.
function Assert-Dependencies([string]$Label, [string[]]$Files, [string[]]$Shipped) {
    $shippedNames = @($Shipped | ForEach-Object { $_.ToLowerInvariant() })
    $used = @{}
    $problems = @()
    foreach ($file in $Files) {
        $lines = & $script:Dumpbin /nologo /dependents $file
        if ($LASTEXITCODE -ne 0) { throw "dumpbin /dependents failed: $file" }
        foreach ($line in $lines) {
            if ($line -notmatch '^\s{4}(\S+\.(dll|exe|drv|sys|ocx|cpl))\s*$') { continue }
            $dll = $Matches[1]
            $used[$dll.ToLowerInvariant()] = $true
            if ($dll -match '^(api|ext)-ms-win-' -or $shippedNames -contains $dll.ToLowerInvariant() -or (Test-SystemDll $dll)) { continue }
            $problems += "  $(Split-Path $file -Leaf) -> $dll"
        }
    }
    if ($problems) {
        throw "$Label uses DLLs that are neither Windows components nor shipped next to it:`n$($problems -join "`n")"
    }
    Write-Host ("{0}: {1} files use {2} DLLs, all from Windows or shipped alongside" -f $Label, $Files.Count, $used.Count)
}

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

    # 설치 폴더마다(cssgsg.iss의 [Files]): {app}에 입력기·호스트, {app}\mozc에 엔진, {app}\settings에 설정 앱 폴더 전체.
    $Dumpbin = Find-Dumpbin
    $appFiles = 'cssgsg_tip.dll', 'cssgsg-host.exe'
    Assert-Dependencies 'cssgsg_tip.dll, cssgsg-host.exe' ($appFiles | ForEach-Object { Join-Path $Root "build\cargo\release\$_" }) $appFiles
    Assert-Dependencies 'mozc\cssgsg_mozc.dll' @(Join-Path $Root 'build\mozc-out\lib\cssgsg_mozc.dll') @('cssgsg_mozc.dll')
    $settings = @(Get-ChildItem (Join-Path $Root 'build\settings\publish') -File | Where-Object { $_.Extension -in '.dll', '.exe' })
    $settingsFiles = @(Get-ChildItem (Join-Path $Root 'build\settings\publish') -File | ForEach-Object { $_.Name })
    Assert-Dependencies 'settings\' ($settings | ForEach-Object { $_.FullName }) $settingsFiles

    & $Iscc /Q "/DVersion=$Version" "/DNumericVersion=$Numeric" "/DRepo=$Root" (Join-Path $Root 'win\installer\cssgsg.iss')
    if ($LASTEXITCODE -ne 0) { throw "ISCC failed (exit code $LASTEXITCODE)" }
}
finally {
    Pop-Location
    $ErrorActionPreference = 'Stop'
}
$Setup = Join-Path $Root 'build\installer\cssgsg-setup.exe'
Write-Host ("cssgsg {0} -> {1} ({2:N1} MB)" -f $Version, $Setup, ((Get-Item $Setup).Length / 1MB))
