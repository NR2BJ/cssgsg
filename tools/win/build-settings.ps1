# 윈도우 설정 앱(win/settings, WinUI 3)을 빌드한다 → build\settings\publish
#
#   powershell -ExecutionPolicy Bypass -File tools\win\build-settings.ps1 0.1.0
#
# - .NET 10 SDK(dotnet)와 MSVC 링커(네이티브 AOT)가 필요하다. 설치기 빌드(build-installer.ps1)가 부른다.
# - 설정 파일 라이브러리(config-ffi → cssgsg_config.dll)를 릴리스로 먼저 빌드한다.
# - 앱에 같이 들어가는 .NET·Windows App SDK·WebView2의 고지문을 NuGet 패키지에서 모아
#   THIRD_PARTY_NOTICES-windows.txt로 둔다(엔진·배열·사전 고지는 저장소의 THIRD_PARTY_NOTICES.txt).
# - 이 파일은 UTF-8(BOM)이다.
param(
    [Parameter(Position = 0)]
    [string]$Version = '0.0.0'
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Out = Join-Path $Root 'build\settings\publish'
$Version = $Version.TrimStart('v')

# 네이티브 AOT의 링커 찾기(findvcvarsall.bat)가 vswhere.exe를 PATH에서도 찾는다.
$installer = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer'
if (Test-Path $installer) { $env:Path = "$installer;$env:Path" }
if (-not (Get-Command dotnet -ErrorAction SilentlyContinue)) {
    $dotnet = Join-Path $env:ProgramFiles 'dotnet'
    if (Test-Path (Join-Path $dotnet 'dotnet.exe')) { $env:Path = "$dotnet;$env:Path" } else { throw 'dotnet (.NET 10 SDK) not found: winget install Microsoft.DotNet.SDK.10' }
}

Push-Location $Root
# cargo·dotnet은 진행 상황을 표준 오류로 낸다. Windows PowerShell 5.1은 그걸 오류로 바꾸니 잠깐 푼다.
$ErrorActionPreference = 'Continue'
try {
    cargo build --release -p cssgsg-config 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE -ne 0) { throw 'cargo build -p cssgsg-config failed' }
    if (Test-Path $Out) { Remove-Item -Recurse -Force $Out }
    dotnet publish win\settings\CssgsgSettings.csproj -c Release -r win-x64 -p:Platform=x64 "-p:Version=$Version" -o $Out 2>&1 | ForEach-Object { "$_" }
    if ($LASTEXITCODE -ne 0) { throw 'dotnet publish failed' }
}
finally {
    Pop-Location
    $ErrorActionPreference = 'Stop'
}
Get-ChildItem $Out -Filter *.pdb | Remove-Item -Force
foreach ($f in 'cssgsg-settings.exe', 'cssgsg-settings.pri', 'cssgsg_config.dll', 'learn\index.html', 'practice\index.html', 'Assets\cssgsg.ico') {
    if (-not (Test-Path (Join-Path $Out $f))) { throw "missing $f in $Out" }
}

# 고지문: 복원한 패키지(project.assets.json)의 정확한 판에서 읽는다.
$assets = Get-Content (Join-Path $Root 'build\settings\obj\project.assets.json') -Raw | ConvertFrom-Json
$packages = $assets.project.restore.packagesPath
function Package([string]$id) {
    $lib = $assets.libraries.PSObject.Properties | Where-Object { $_.Name -like "$id/*" } | Select-Object -First 1
    if (-not $lib) { throw "package $id is not restored" }
    [pscustomobject]@{ Version = $lib.Name.Split('/')[1]; Dir = Join-Path $packages $lib.Value.path }
}
$sections = New-Object System.Collections.Generic.List[string]
function Section([string]$title, [string]$url, [string[]]$files) {
    $text = ($files | ForEach-Object { [IO.File]::ReadAllText($_).Trim() }) -join "`n`n"
    $rule = '=' * 80
    $sections.Add("$rule`n$title`n$url`n$rule`n`n$text`n")
}
$ilc = Package 'runtime.win-x64.Microsoft.DotNet.ILCompiler'
$dotnetLicense = @'
The MIT License (MIT)

Copyright (c) .NET Foundation and Contributors

All rights reserved.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
'@
$tmp = [IO.Path]::GetTempFileName()
[IO.File]::WriteAllText($tmp, $dotnetLicense)
Section ".NET runtime (native AOT) $($ilc.Version)" 'https://github.com/dotnet/runtime' @($tmp, (Join-Path $ilc.Dir 'THIRD-PARTY-NOTICES.TXT'))
Remove-Item $tmp
$winui = Package 'Microsoft.WindowsAppSDK.WinUI'
$base = Package 'Microsoft.WindowsAppSDK.Base'
Section "Windows App SDK (WinUI $($winui.Version), Base $($base.Version))" 'https://github.com/microsoft/WindowsAppSDK' @(
    (Join-Path $winui.Dir 'license.txt'), (Join-Path $winui.Dir 'NOTICE.txt'), (Join-Path $base.Dir 'NOTICE.txt'))
$webview = Package 'Microsoft.Web.WebView2'
Section "Microsoft Edge WebView2 SDK $($webview.Version)" 'https://aka.ms/webview2' @(
    (Join-Path $webview.Dir 'LICENSE.txt'), (Join-Path $webview.Dir 'NOTICE.txt'))
$header = "cssgsg 윈도우 설정 앱에 들어 있는 다른 소프트웨어의 저작권·라이선스 고지.`nThird-party notices for software included in the cssgsg settings app for Windows.`n`n"
[IO.File]::WriteAllText((Join-Path $Out 'THIRD_PARTY_NOTICES-windows.txt'), $header + ($sections -join "`n"), (New-Object Text.UTF8Encoding $false))

$size = (Get-ChildItem $Out -Recurse -File | Measure-Object Length -Sum).Sum / 1MB
Write-Host ("cssgsg settings {0} -> {1} ({2:N1} MB)" -f $Version, $Out, $size)
