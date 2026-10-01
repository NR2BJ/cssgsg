# 개발용 입력기 설치·제거(윈도우 VM). 관리자 권한 창(UAC)이 한 번 뜬다.
#
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 install     빌드 → Program Files\cssgsg에 복사 → 등록 → 내 입력 목록에 추가
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 uninstall   입력 목록에서 빼기 → 등록 해제 → 파일 지우기
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 status      등록·목록·파일 상태만 본다
#
# - DLL은 앱 컨테이너 앱(새 메모장, 시작 메뉴 검색)도 읽을 수 있는 Program Files에 둔다.
# - 앱이 쥐고 있는 DLL은 지울 수 없어서 이름을 바꿔 두고 새 파일을 넣는다(이미 뜬 앱은 옛 DLL을 계속 쓴다).
# - 처음 등록하기 전에는 VM 스냅숏을 찍는다(망가진 입력기는 탐색기까지 끌고 간다).
# - 이 파일은 UTF-8(BOM)이다. Windows PowerShell 5.1은 BOM 없는 스크립트를 ANSI(CP949)로 읽는다.
param(
    [Parameter(Mandatory = $true, Position = 0)][ValidateSet('install', 'uninstall', 'status')][string]$Action,
    [switch]$Elevated,
    [string]$Dll,
    [string]$Log
)
$ErrorActionPreference = 'Stop'

$Clsid = '{A9227DC2-56BC-4023-AE29-82A9AAA4EE14}'    # win/tip/src/lib.rs CLSID_TEXT_SERVICE
$ProfileGuid = '{DCFBD969-D52F-4AFB-ABC0-271CC58FE18C}'  # GUID_PROFILE
$Tip = "0x0409:$Clsid$ProfileGuid"                       # InstallLayoutOrTip 형식(en-US)
$InstallDir = Join-Path $env:ProgramFiles 'cssgsg'
$Target = Join-Path $InstallDir 'cssgsg_tip.dll'
$Repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Write-Step($m) { Write-Host "== $m" }

# ---- 관리자 쪽(같은 스크립트를 -Elevated로 다시 띄운다) -------------------------------------------
if ($Elevated) {
    function Write-Host($m) { Add-Content -Path $Log -Value $m -Encoding UTF8 }
    # 지우거나, 쓰는 중이면 이름을 바꿔 두고 다시 시작할 때 지우게 예약한다(이름을 먼저 바꿔야 새로 넣은 DLL이 함께 지워지지 않는다).
    function Remove-OrLater([string]$path) {
        try { Remove-Item $path -Force; return } catch { }
        if ((Split-Path $path -Leaf) -notmatch '\.old-\d+$') {
            $old = "$path.old-$([DateTime]::Now.Ticks)"
            Rename-Item $path (Split-Path $old -Leaf)
            $path = $old
        }
        if (-not ('Cssgsg.Native' -as [type])) {
            Add-Type -Namespace Cssgsg -Name Native -MemberDefinition '[DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)] public static extern bool MoveFileExW(string from, string to, uint flags);'
        }
        # 새 이름은 진짜 null이어야 한다(PowerShell은 $null을 빈 문자열로 넘긴다).
        if ([Cssgsg.Native]::MoveFileExW($path, [NullString]::Value, 4)) {   # MOVEFILE_DELAY_UNTIL_REBOOT
            Write-Host "쓰는 중이라 다시 시작할 때 지움: $(Split-Path $path -Leaf)"
        }
        else { Write-Host "다시 시작 때 지우기 예약 실패(오류 $([Runtime.InteropServices.Marshal]::GetLastWin32Error())): $(Split-Path $path -Leaf)" }
    }
    Set-Content -Path $Log -Value "관리자 단계: $Action" -Encoding UTF8
    $code = 0
    try {
        if ($Action -eq 'install') {
            New-Item -ItemType Directory -Force $InstallDir | Out-Null
            Get-ChildItem $InstallDir -Filter 'cssgsg_tip.dll.old-*' -ErrorAction SilentlyContinue |
                ForEach-Object { Remove-OrLater $_.FullName }
            if (Test-Path $Target) { Remove-OrLater $Target }
            Copy-Item $Dll $Target
            $p = Start-Process regsvr32.exe -ArgumentList '/s', "`"$Target`"" -Wait -PassThru
            Write-Host "regsvr32 종료 코드 $($p.ExitCode)"
            $code = $p.ExitCode
        }
        elseif ($Action -eq 'uninstall') {
            if (Test-Path $Target) {
                $p = Start-Process regsvr32.exe -ArgumentList '/u', '/s', "`"$Target`"" -Wait -PassThru
                Write-Host "regsvr32 /u 종료 코드 $($p.ExitCode)"
                $code = $p.ExitCode
            }
            foreach ($f in Get-ChildItem $InstallDir -Filter 'cssgsg_tip.dll*' -ErrorAction SilentlyContinue) { Remove-OrLater $f.FullName }
            if (Test-Path $InstallDir) {
                if (-not (Get-ChildItem $InstallDir)) { Remove-Item $InstallDir -Force }
                # 남은 파일이 다시 시작 때 지워진 뒤 폴더도(예약은 적은 차례대로 처리된다).
                elseif ('Cssgsg.Native' -as [type]) { [void][Cssgsg.Native]::MoveFileExW($InstallDir, [NullString]::Value, 4) }
            }
        }
    }
    catch { Write-Host "오류: $_"; $code = 1 }
    exit $code
}

# ---- 사용자 쪽 ----------------------------------------------------------------------------------
function Invoke-Tip([uint32]$flags) {
    if (-not ('Cssgsg.Input' -as [type])) {
        Add-Type -Namespace Cssgsg -Name Input -MemberDefinition @'
[DllImport("input.dll", CharSet = CharSet.Unicode, SetLastError = true)]
public static extern bool InstallLayoutOrTip(string psz, uint dwFlags);
'@
    }
    if (-not [Cssgsg.Input]::InstallLayoutOrTip($Tip, $flags)) {
        throw "InstallLayoutOrTip 실패 (오류 $([Runtime.InteropServices.Marshal]::GetLastWin32Error()))"
    }
}

function Invoke-Elevated([string]$what, [string]$dll) {
    $log = Join-Path $Repo 'build\tip-dev-elevated.log'
    New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
    $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", $what, '-Elevated', '-Log', "`"$log`"")
    if ($dll) { $argList += @('-Dll', "`"$dll`"") }
    Write-Step '관리자 권한 창이 뜨면 승인해 주세요'
    $p = Start-Process powershell.exe -Verb RunAs -ArgumentList $argList -Wait -PassThru
    if (Test-Path $log) { Get-Content $log -Encoding UTF8 | ForEach-Object { "   $_" } }
    if ($p.ExitCode -ne 0) { throw "관리자 단계 실패 (종료 코드 $($p.ExitCode))" }
}

# ILOT_UNINSTALL 뒤에도 사용자 TSF 정렬 캐시(SortOrder\AssemblyItem\<언어>)에 우리 항목이 남는다.
# 그 언어의 항목이 모두 우리 것일 때만 언어째 지운다(남의 입력기 순서는 건드리지 않는다).
function Remove-StaleSortOrder {
    $base = 'HKCU:\Software\Microsoft\CTF\SortOrder\AssemblyItem'
    foreach ($lang in @(Get-ChildItem $base -ErrorAction SilentlyContinue)) {
        $items = @(Get-ChildItem $lang.PSPath -Recurse | Where-Object { $_.GetValue('CLSID') })
        $ours = @($items | Where-Object { $_.GetValue('CLSID') -eq $Clsid })
        if ($items.Count -gt 0 -and $ours.Count -eq $items.Count) {
            Remove-Item $lang.PSPath -Recurse -Force
            Write-Host "   정렬 캐시에서 뺌: $($lang.PSChildName)"
        }
    }
}

function Show-Status {
    $reg = Test-Path "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Classes\CLSID\$Clsid\InprocServer32"
    $ctf = Test-Path "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\CTF\TIP\$Clsid"
    $langs = Get-WinUserLanguageList   # List<> 하나로 오니 변수에 받아서 돈다
    $listed = @(foreach ($l in $langs) { if ($l.InputMethodTips -match [regex]::Escape($Clsid.Trim('{}'))) { $l } }).Count -gt 0
    Write-Host ("CLSID 등록: {0}  TSF 프로필: {1}  내 입력 목록: {2}  파일: {3}" -f $reg, $ctf, $listed, (Test-Path $Target))
    foreach ($l in $langs) { Write-Host ("  {0}: {1}" -f $l.LanguageTag, ($l.InputMethodTips -join ', ')) }
}

switch ($Action) {
    'install' {
        Write-Step '릴리스 빌드'
        Push-Location $Repo
        # cargo는 진행 상황을 표준 오류로 낸다. Windows PowerShell 5.1은 그걸 오류로 바꾸니(출력을 받아 갈 때) 잠깐 푼다.
        $ErrorActionPreference = 'Continue'
        try { cargo build --release -p cssgsg-tip 2>&1 | ForEach-Object { "$_" }; $built = $LASTEXITCODE -eq 0 }
        finally { Pop-Location; $ErrorActionPreference = 'Stop' }
        if (-not $built) { throw 'cargo build 실패' }
        Invoke-Elevated 'install' (Join-Path $Repo 'build\cargo\release\cssgsg_tip.dll')
        Write-Step '내 입력 목록에 추가(en-US)'
        Invoke-Tip 0
        Show-Status
    }
    'uninstall' {
        Write-Step '내 입력 목록에서 빼기'
        try { Invoke-Tip 1 } catch { Write-Host $_ }   # ILOT_UNINSTALL
        Remove-StaleSortOrder
        # TSF가 사용자별로 남기는 우리 입력기 키(LanguageProfile\…\Enable=0).
        Remove-Item "HKCU:\Software\Microsoft\CTF\TIP\$Clsid" -Recurse -Force -ErrorAction SilentlyContinue
        Invoke-Elevated 'uninstall' $null
        Show-Status
    }
    'status' { Show-Status }
}
