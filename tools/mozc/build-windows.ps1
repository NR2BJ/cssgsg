# 윈도우에서 Mozc(일본어 변환 엔진)를 cssgsg용으로 빌드한다 → build/mozc-out/ (tools/mozc/build.sh의 윈도우판, 같은 모양)
#   lib/cssgsg_mozc.dll       엔진 호스트가 실행 중에 읽는 엔진(Mozc와 의존성 전부 + 우리 C API, C 런타임 정적 링크)
#   include/cssgsg_mozc.h     C API
#   data/mozc.data            사전 데이터(OSS)
#   bin/cssgsg_mozc_main.exe  확인용 명령줄 도구
#   MOZC_VERSION              "<커밋> <커밋 날짜> <Mozc 버전> <래퍼 판>"(엔진 호스트가 설치본에 든 엔진의 판을 안다)
#                             래퍼 판은 mozc/cssgsg/WRAPPER_REVISION(맥 build.sh와 같다, CONCEPT §6.3).
#
# - Mozc 커밋은 tools/mozc/MOZC_COMMIT에 고정한다. 환경 변수 MOZC_COMMIT이 있으면 그것. MOZC_BAZEL_FLAGS로 Bazel 옵션을 더한다
#   (워크플로의 --disk_cache 등).
# - 필요한 것: Visual Studio 2022 또는 2026(C++ 데스크톱, Windows SDK), Python 3.12+, bazelisk, git.
#   컴파일러는 Mozc가 받는 clang-cl(LLVM)이고, 데이터를 만드는 bash(MSYS2)도 Mozc가 받는다(build_tools/update_deps.py).
#   GitHub windows-2025 러너에는 다 있다(.github/workflows/mozc-windows.yml).
# - Bazel은 심볼릭 링크를 쓴다. 로컬 빌드는 윈도우 개발자 모드가 켜져 있어야 한다(러너는 관리자라 된다).
# - C 런타임은 모든 대상에 정적으로 링크한다(--features=static_link_msvcrt). 대상 하나에만 주면 링크가 깨진다.
# - 이 파일은 UTF-8(BOM)이다. Windows PowerShell 5.1은 BOM 없는 스크립트를 ANSI(CP949)로 읽는다.
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Commit = if ($env:MOZC_COMMIT) { $env:MOZC_COMMIT.Trim() } else { (Get-Content (Join-Path $Root 'tools\mozc\MOZC_COMMIT') -Raw).Trim() }
$Src = Join-Path $Root 'build\mozc'
$Out = Join-Path $Root 'build\mozc-out'

# 바깥 명령을 돌리고, 실패하면 멈춘다.
function Invoke-Native([string]$What, [scriptblock]$Run) {
    & $Run
    if ($LASTEXITCODE -ne 0) { throw "$What failed (exit code $LASTEXITCODE)" }
}

if (-not (Get-Command bazelisk -ErrorAction SilentlyContinue)) { throw 'bazelisk not found' }

# 워크플로가 캐시(src/third_party_cache)를 먼저 풀어 둘 수 있어서 clone 대신 init + fetch를 쓴다.
if (-not (Test-Path (Join-Path $Src '.git'))) { Invoke-Native 'git init' { git init -q $Src } }
$head = git -C $Src rev-parse --verify -q HEAD
if ($head -ne $Commit) {
    Invoke-Native 'git fetch' { git -C $Src fetch -q --depth 1 https://github.com/google/mozc.git $Commit }
    Invoke-Native 'git checkout' { git -C $Src checkout -q --detach FETCH_HEAD }
}

# 우리 C API를 Mozc 트리의 src/cssgsg로 복사한다(build.sh의 rsync --delete와 같다).
$Wrapper = Join-Path $Src 'src\cssgsg'
if (Test-Path $Wrapper) { Remove-Item -Recurse -Force $Wrapper }
Copy-Item -Recurse (Join-Path $Root 'mozc\cssgsg') $Wrapper

Push-Location (Join-Path $Src 'src')
try {
    # Qt·WiX·NDK·Ninja는 GUI와 설치기용이라 받지 않는다(LLVM과 MSYS2만).
    Invoke-Native 'update_deps.py' { python build_tools/update_deps.py --noqt --nowix --nondk --noninja }

    # Mozc는 세션 처리기 등을 정해진 패키지에만 보이게 해 둔다. 소스를 고치지 않고 검사만 끈다(build.sh와 같다).
    $flags = @('--config', 'oss_windows', '--config', 'release_build', '--nocheck_visibility', '--features=static_link_msvcrt')
    if ($env:MOZC_BAZEL_FLAGS) { $flags += @($env:MOZC_BAZEL_FLAGS -split '\s+' | Where-Object { $_ }) }
    $targets = @('//cssgsg:cssgsg_mozc.dll', '//cssgsg:cssgsg_mozc_main', '//data_manager/oss:mozc.data')
    Invoke-Native 'bazelisk build' { bazelisk build @flags @targets }
    # 다음 빌드(같은 워크플로의 cargo 등)와 디스크 캐시 저장이 Bazel 서버에 묶이지 않게 끈다.
    bazelisk shutdown | Out-Null

    $Bin = Join-Path (Get-Location) 'bazel-bin'
    if (Test-Path (Join-Path $Out 'lib')) { Remove-Item -Recurse -Force (Join-Path $Out 'lib') }
    foreach ($dir in 'lib', 'include', 'data', 'bin') { New-Item -ItemType Directory -Force (Join-Path $Out $dir) | Out-Null }
    Copy-Item -Force (Join-Path $Bin 'cssgsg\cssgsg_mozc.dll') (Join-Path $Out 'lib')
    Copy-Item -Force (Join-Path $Root 'mozc\cssgsg\cssgsg_mozc.h') (Join-Path $Out 'include')
    Copy-Item -Force (Join-Path $Bin 'data_manager\oss\mozc.data') (Join-Path $Out 'data')
    Copy-Item -Force (Join-Path $Bin 'cssgsg\cssgsg_mozc_main.exe') (Join-Path $Out 'bin')
    # Bazel이 만든 파일은 읽기 전용이다.
    Get-ChildItem $Out -Recurse -File | ForEach-Object { $_.IsReadOnly = $false }

    $v = @{}
    Get-Content 'version.bzl' | ForEach-Object { if ($_ -match '^(\w+) = (\d+)') { $v[$Matches[1]] = [int]$Matches[2] } }
    $Version = '{0}.{1}.{2}.{3}' -f $v['MAJOR'], $v['MINOR'], $v['BUILD_OSS'], ($v['REVISION'] + 1)
    $Date = git -C $Src show -s --format=%cs HEAD
    $Wrapper = (Get-Content (Join-Path $Root 'mozc\cssgsg\WRAPPER_REVISION') -Raw).Trim()
    if ($Wrapper -notmatch '^\d+$') { throw "mozc/cssgsg/WRAPPER_REVISION must be a number: $Wrapper" }
    Set-Content -Path (Join-Path $Out 'MOZC_VERSION') -Value "$Commit $Date $Version $Wrapper" -Encoding Ascii
    Write-Host "Mozc $Version ($Commit, $Date, wrapper $Wrapper) -> $Out"
    Get-ChildItem (Join-Path $Out 'lib'), (Join-Path $Out 'data'), (Join-Path $Out 'bin') | Format-Table Name, Length
}
finally {
    Pop-Location
}
