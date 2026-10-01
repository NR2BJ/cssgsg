# build/mozc-out(build-windows.ps1)을 윈도우 엔진 묶음으로 만든다(package-component.sh의 윈도우판):
#   build/mozc-component/cssgsg-mozc-windows-x64.zip   cssgsg_mozc.dll, mozc.data, manifest.json
#   build/mozc-component/TAG, TITLE                     맥 묶음과 같은 태그·제목(같은 Mozc 커밋이면 한 릴리스에 같이 싣는다)
# manifest.json은 맥 것과 같은 모양에 "platform"을 더했다: C API 판, Mozc 커밋·날짜·버전, 파일마다 SHA-256.
# 맥 입력기(MozcUpdater)는 이름이 cssgsg-mozc.zip인 것만 받으므로 이 묶음은 보지 않는다.
# 이 파일은 UTF-8(BOM)이다.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Out = Join-Path $Root 'build\mozc-out'
$Dest = Join-Path $Root 'build\mozc-component'
$Zip = Join-Path $Dest 'cssgsg-mozc-windows-x64.zip'

$Commit, $Date, $Version = (Get-Content (Join-Path $Out 'MOZC_VERSION') -Raw).Trim() -split ' '
$Abi = (Select-String -Path (Join-Path $Root 'mozc\cssgsg\cssgsg_mozc.h') -Pattern '^#define CSSGSG_MOZC_ABI_VERSION (\d+)').Matches |
    Select-Object -First 1 | ForEach-Object { $_.Groups[1].Value }
if (-not $Abi) { throw 'CSSGSG_MOZC_ABI_VERSION not found' }

$Stage = Join-Path $Dest 'stage-windows'
if (Test-Path $Stage) { Remove-Item -Recurse -Force $Stage }
New-Item -ItemType Directory -Force $Stage | Out-Null
Copy-Item (Join-Path $Out 'lib\cssgsg_mozc.dll'), (Join-Path $Out 'data\mozc.data') $Stage
$sha = { param($file) (Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant() }
$DllSha = & $sha (Join-Path $Stage 'cssgsg_mozc.dll')
$DataSha = & $sha (Join-Path $Stage 'mozc.data')
$Manifest = "{`"abi`": $Abi, `"commit`": `"$Commit`", `"date`": `"$Date`", `"version`": `"$Version`", `"platform`": `"windows-x64`",`n" +
    " `"files`": {`"cssgsg_mozc.dll`": `"$DllSha`", `"mozc.data`": `"$DataSha`"}}`n"
[IO.File]::WriteAllText((Join-Path $Stage 'manifest.json'), $Manifest, (New-Object Text.UTF8Encoding $false))
if (Test-Path $Zip) { Remove-Item -Force $Zip }
Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Zip
Remove-Item -Recurse -Force $Stage

$Tag = "mozc-$Abi-$($Date.Replace('-', ''))-$($Commit.Substring(0, 7))"
[IO.File]::WriteAllText((Join-Path $Dest 'TAG'), "$Tag`n")
[IO.File]::WriteAllText((Join-Path $Dest 'TITLE'), "Mozc $Version ($Date)`n")
Write-Host "${Tag}: Mozc $Version ($Date) -> $Zip ($([math]::Round((Get-Item $Zip).Length / 1MB, 1)) MB)"
