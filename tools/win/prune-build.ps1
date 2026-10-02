# 빌드 결과물(build\cargo)에서 지난 빌드의 옛 판을 지운다(사용자: "한번 빌드 하면 최신 빌드 제외 과거 빌드는 지우게 하자", 2026-10-02).
#
#   powershell -ExecutionPolicy Bypass -File tools\win\prune-build.ps1 [-Hours 24]
#
# - cargo는 같은 크레이트라도 빌드 방식(시험·검사·릴리스, 기능, 컴파일러 판)마다 다른 해시로 결과물을 따로 두고 옛 것을 지우지 않는다
#   (2026-10-02 VM: windows 크레이트 32벌, 증분 기록 241개, 11GB). 결과물 한 벌은 deps의 파일과 .fingerprint·build·incremental의 폴더로,
#   이름 끝의 16자리 해시가 같다. 한 벌의 어느 파일도 -Hours(기본 24시간) 동안 쓰이지 않았으면(읽거나 쓰지 않았으면) 한 벌을 통째로 지운다.
#   한 벌의 일부만 지우지는 않는다. 지금 쓰는 판은 cargo가 볼 때마다 .fingerprint를 읽어서 남는다.
# - 그날 쓴 것까지 지우지 않는 까닭: 시험(디버그)·검사(clippy)·개발 설치(릴리스)를 오갈 때마다 windows 크레이트 등을 몇 분씩 다시 컴파일하게 된다.
# - 쓰였는지는 NTFS 마지막 접근 시각으로 본다(이 VM은 갱신 켬, 한 시간 단위로 늦게 적힐 수 있다). 갱신을 끈 PC라면 그날 다시 컴파일하지
#   않은 판이 지워져 다음 빌드가 조금 느려질 뿐이다.
# - 증분 컴파일 기록은 이 VM의 사용자 cargo 설정(%USERPROFILE%\.cargo\config.toml, build.incremental = false)으로 아예 만들지 않는다.
# - tip-dev.ps1 install과 build-installer.ps1이 끝에 부른다. 손으로 cargo를 돌린 뒤에도 부른다.
# - 이 파일은 UTF-8(BOM)이다.
param([double]$Hours = 24)
$ErrorActionPreference = 'Stop'
$Target = Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path 'build\cargo'
if (-not (Test-Path $Target)) { return }
$cutoff = (Get-Date).AddHours(-$Hours)

# 마지막으로 쓰인 때(폴더는 안의 파일 중 가장 늦은 것).
function Get-LastUse([IO.FileSystemInfo]$item) {
    $files = if ($item -is [IO.DirectoryInfo]) { @(Get-ChildItem -LiteralPath $item.FullName -Recurse -File -Force -ErrorAction SilentlyContinue) } else { @($item) }
    $times = @($item.LastWriteTime) + @($files | ForEach-Object { $_.LastAccessTime; $_.LastWriteTime })
    ($times | Measure-Object -Maximum).Maximum
}

function Get-Size { (Get-ChildItem -LiteralPath $Target -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum / 1GB }

$before = Get-Size
$removed = 0
# 빌드 방식마다 한 폴더(debug, release, <대상>\debug …): .fingerprint가 있는 곳.
$profiles = Get-ChildItem -LiteralPath $Target -Recurse -Directory -Force -Filter '.fingerprint' | ForEach-Object { $_.Parent }
foreach ($profile in $profiles) {
    $sets = @{}
    foreach ($sub in 'deps', '.fingerprint', 'build', 'incremental') {
        $dir = Join-Path $profile.FullName $sub
        if (-not (Test-Path -LiteralPath $dir)) { continue }
        foreach ($item in Get-ChildItem -LiteralPath $dir -Force) {
            if ($item.Name -match '-([0-9a-f]{16})(\.|$)') { $sets[$Matches[1]] += @($item) }
        }
    }
    foreach ($hash in @($sets.Keys)) {
        $items = $sets[$hash]
        $last = ($items | ForEach-Object { Get-LastUse $_ } | Measure-Object -Maximum).Maximum
        if ($last -lt $cutoff) {
            foreach ($item in $items) { Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction SilentlyContinue }
            $removed++
        }
    }
}
Write-Host ("build\cargo: {0} old builds removed ({1:N2} -> {2:N2} GB, unused for {3} h)" -f $removed, $before, (Get-Size), $Hours)
