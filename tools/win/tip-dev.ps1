# 개발용 입력기 설치·제거(윈도우 VM). 관리자 권한 창(UAC)이 한 번 뜬다.
#
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 install       빌드 → Program Files\cssgsg에 복사 → 등록 → 내 입력 목록에 추가
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 uninstall     입력 목록에서 빼기 → 등록 해제 → 파일 지우기
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 status        등록·목록·파일 상태만 본다
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 debuglog-on   개발자 기록 켜기(%LOCALAPPDATA%\cssgsg\tip-debug.log, 키 코드만)
#   powershell -ExecutionPolicy Bypass -File tools\win\tip-dev.ps1 debuglog-off
#
# - DLL은 앱 컨테이너 앱(새 메모장, 시작 메뉴 검색)도 읽을 수 있는 Program Files에 둔다.
# - 엔진 호스트(cssgsg-host.exe, 일본어 한자 변환)도 같은 폴더에, Mozc 엔진은 그 아래 mozc 폴더에 둔다. 엔진은 build\mozc-out에
#   있을 때만 넣는다(tools\mozc\build-windows.ps1 또는 워크플로 mozc-windows.yml의 묶음을 풀어 둔 것). 설치할 때 떠 있는 호스트는 끈다.
#   호스트는 로그인부터 늘 켜 둔다: enable이 시작 프로그램(HKCU Run의 cssgsg)에 넣고 바로 띄우며, disable이 빼고 끈다.
# - 앱이 쥐고 있는 DLL은 지울 수 없어서 이름을 바꿔 두고 새 파일을 넣는다(이미 뜬 앱은 옛 DLL을 계속 쓴다). 옛 파일은 다시 시작할 때 지운다.
# - 처음 등록하기 전에는 VM 스냅숏을 찍는다(망가진 입력기는 탐색기까지 끌고 간다).
# - 사용자 쪽 단계(입력 목록, HKCU 정리, 기록 설정)는 WMI로 띄운 프로세스에서 한다. 이 스크립트를 MSIX 패키지 앱
#   (Claude 데스크톱 앱 등) 안의 셸에서 돌리면 HKCU\Software와 %LOCALAPPDATA% 쓰기가 그 앱의 칸으로 옮겨져서(가상화)
#   다른 앱의 입력기가 보지 못한다. 관리자 단계는 원래 패키지 밖에서 돈다.
# - 이 파일은 UTF-8(BOM)이다. Windows PowerShell 5.1은 BOM 없는 스크립트를 ANSI(CP949)로 읽는다.
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('install', 'uninstall', 'status', 'debuglog-on', 'debuglog-off', 'enable', 'disable')]
    [string]$Action,
    [switch]$Elevated,
    [switch]$Outside,
    [string]$Dll,
    [string]$Log
)
$ErrorActionPreference = 'Stop'

$Clsid = '{A9227DC2-56BC-4023-AE29-82A9AAA4EE14}'        # win/tip/src/lib.rs CLSID_TEXT_SERVICE
$ProfileGuid = '{DCFBD969-D52F-4AFB-ABC0-271CC58FE18C}'  # GUID_PROFILE
$Tip = "0x0412:$Clsid$ProfileGuid"                       # InstallLayoutOrTip 형식(한국어). 0.2.7까지는 0x0409(en-US)
$InstallDir = Join-Path $env:ProgramFiles 'cssgsg'
$Target = Join-Path $InstallDir 'cssgsg_tip.dll'
$HostExe = Join-Path $InstallDir 'cssgsg-host.exe'      # 엔진 호스트(win/host)
$MozcDir = Join-Path $InstallDir 'mozc'                 # 호스트가 읽는 Mozc 엔진(cssgsg_mozc.dll, mozc.data)
$RunKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'   # 호스트를 로그인 때 띄우는 시작 프로그램(값 이름 cssgsg)
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
            # 엔진 호스트(일본어 한자 변환): 떠 있으면 끈다(다음 변환 때 입력기가 새것을 띄운다).
            Get-Process cssgsg-host -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Get-ChildItem $InstallDir -Filter 'cssgsg-host.exe.old-*' -ErrorAction SilentlyContinue |
                ForEach-Object { Remove-OrLater $_.FullName }
            if (Test-Path $HostExe) { Remove-OrLater $HostExe }
            Copy-Item (Join-Path (Split-Path $Dll) 'cssgsg-host.exe') $HostExe
            # Mozc 엔진: 빌드했거나(tools\mozc\build-windows.ps1) 워크플로 묶음을 풀어 둔 build\mozc-out. 없으면 히라가나·가타카나만.
            $engine = Join-Path $Repo 'build\mozc-out'
            if (Test-Path (Join-Path $engine 'lib\cssgsg_mozc.dll')) {
                New-Item -ItemType Directory -Force $MozcDir | Out-Null
                foreach ($f in Get-ChildItem $MozcDir -File -ErrorAction SilentlyContinue) { Remove-OrLater $f.FullName }
                Copy-Item (Join-Path $engine 'lib\cssgsg_mozc.dll'), (Join-Path $engine 'data\mozc.data') $MozcDir
                if (Test-Path (Join-Path $engine 'MOZC_VERSION')) { Copy-Item (Join-Path $engine 'MOZC_VERSION') $MozcDir }
                Write-Host "Mozc 엔진: $((Get-Content (Join-Path $engine 'MOZC_VERSION') -ErrorAction SilentlyContinue) -join ' ')"
            }
            else { Write-Host 'Mozc 엔진 없음(build\mozc-out): 일본어 변환은 히라가나·가타카나만' }
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
            # 엔진 호스트와 Mozc 엔진. 학습·사용자 사전(%LOCALAPPDATA%\cssgsg\mozc)은 남긴다(맥과 같다).
            Get-Process cssgsg-host -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            foreach ($f in Get-ChildItem $InstallDir -Filter 'cssgsg-host.exe*' -ErrorAction SilentlyContinue) { Remove-OrLater $f.FullName }
            if (Test-Path $MozcDir) {
                foreach ($f in Get-ChildItem $MozcDir -File) { Remove-OrLater $f.FullName }
                if (-not (Get-ChildItem $MozcDir)) { Remove-Item $MozcDir -Force }
            }
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

# ---- 사용자 쪽, 패키지 밖(WMI로 띄운 프로세스) ------------------------------------------------------
if ($Outside) {
    function Write-Host($m) { Add-Content -Path $Log -Value $m -Encoding UTF8 }
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
    Set-Content -Path $Log -Value '' -Encoding UTF8
    $code = 0
    try {
        switch ($Action) {
            'enable' {
                # 설치기와 같은 코드(cssgsg-host.exe --install-user, win/host/src/setup.rs): 내 입력 목록에 넣고, 엔진 호스트를
                # 시작 프로그램에 넣고 띄운다(스토어 앱·관리자 앱은 호스트를 띄울 수 없어서 로그인부터 늘 켜 둔다).
                if (-not (Test-Path $HostExe)) { throw "$HostExe 없음" }
                # -Wait는 자식까지 기다려서(--install-user가 띄운 호스트는 늘 떠 있다) 이 단계가 끝나지 않았다: 그 프로세스만 기다린다.
                $p = Start-Process $HostExe -ArgumentList '--install-user' -PassThru
                $p.WaitForExit()
                Write-Host "cssgsg-host --install-user(입력 목록, 시작 프로그램, 호스트): 종료 코드 $($p.ExitCode)"
            }
            'disable' {
                # 설치기와 같은 코드(--uninstall-user). 실행 파일이 없을 때만 아래 PowerShell로 한다.
                if (Test-Path $HostExe) {
                    $p = Start-Process $HostExe -ArgumentList '--uninstall-user' -Wait -PassThru
                    Write-Host "cssgsg-host --uninstall-user(입력 목록, 정렬 캐시, 사용자 TSF 키, 시작 프로그램, 호스트): 종료 코드 $($p.ExitCode)"
                    break
                }
                try { Invoke-Tip 1; Write-Host '내 입력 목록에서 뺌' } catch { Write-Host "$_" }   # ILOT_UNINSTALL
                Remove-ItemProperty -Path $RunKey -Name 'cssgsg' -ErrorAction SilentlyContinue
                Get-Process cssgsg-host -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
                Write-Host '엔진 호스트: 시작 프로그램에서 빼고 끔'
                # ILOT_UNINSTALL 뒤에도 사용자 TSF 정렬 캐시(SortOrder\AssemblyItem\<언어>)에 우리 항목이 남는다.
                # 그 언어의 항목이 모두 우리 것일 때만 언어째 지운다(남의 입력기 순서는 건드리지 않는다).
                $base = 'HKCU:\Software\Microsoft\CTF\SortOrder\AssemblyItem'
                foreach ($lang in @(Get-ChildItem $base -ErrorAction SilentlyContinue)) {
                    $items = @(Get-ChildItem $lang.PSPath -Recurse | Where-Object { $_.GetValue('CLSID') })
                    $ours = @($items | Where-Object { $_.GetValue('CLSID') -eq $Clsid })
                    if ($items.Count -gt 0 -and $ours.Count -eq $items.Count) {
                        Remove-Item $lang.PSPath -Recurse -Force
                        Write-Host "정렬 캐시에서 뺌: $($lang.PSChildName)"
                    }
                }
                # TSF가 사용자별로 남기는 우리 입력기 키(LanguageProfile\…\Enable=0).
                if (Test-Path "HKCU:\Software\Microsoft\CTF\TIP\$Clsid") {
                    Remove-Item "HKCU:\Software\Microsoft\CTF\TIP\$Clsid" -Recurse -Force
                    Write-Host '사용자 TSF 키를 지움'
                }
            }
            'debuglog-on' {
                New-Item -Path 'HKCU:\Software\cssgsg' -Force | Out-Null
                New-ItemProperty -Path 'HKCU:\Software\cssgsg' -Name DebugLog -PropertyType DWord -Value 1 -Force | Out-Null
                Write-Host "개발자 기록 켬: $env:LOCALAPPDATA\cssgsg\tip-debug.log (입력칸을 옮기면 따른다)"
            }
            'debuglog-off' {
                Remove-ItemProperty -Path 'HKCU:\Software\cssgsg' -Name DebugLog -ErrorAction SilentlyContinue
                Write-Host '개발자 기록 끔'
            }
            'status' {
                $reg = Test-Path "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Classes\CLSID\$Clsid\InprocServer32"
                $ctf = Test-Path "Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\CTF\TIP\$Clsid"
                $langs = Get-WinUserLanguageList   # List<> 하나로 오니 변수에 받아서 돈다
                $listed = @(foreach ($l in $langs) { if ($l.InputMethodTips -match [regex]::Escape($Clsid.Trim('{}'))) { $l } }).Count -gt 0
                $debug = (Get-ItemProperty 'HKCU:\Software\cssgsg' -Name DebugLog -ErrorAction SilentlyContinue).DebugLog -eq 1
                Write-Host ("CLSID 등록: {0}  TSF 프로필: {1}  내 입력 목록: {2}  파일: {3}  개발자 기록: {4}" -f $reg, $ctf, $listed, (Test-Path $Target), $debug)
                foreach ($l in $langs) { Write-Host ("  {0}: {1}" -f $l.LanguageTag, ($l.InputMethodTips -join ', ')) }
                $left = @((Test-Path "HKCU:\Software\Microsoft\CTF\TIP\$Clsid"), (Test-Path 'HKCU:\Software\Microsoft\CTF\SortOrder\AssemblyItem\0x00000409'))
                Write-Host ("  사용자 TSF 키: {0}  옛 en-US 정렬 캐시: {1}" -f $left[0], $left[1])
                $running = @(Get-Process cssgsg-host -ErrorAction SilentlyContinue).Count
                $startup = $null -ne (Get-ItemProperty $RunKey -Name 'cssgsg' -ErrorAction SilentlyContinue)
                Write-Host ("  엔진 호스트: {0}  Mozc 엔진: {1}  시작 프로그램: {2}  떠 있는 호스트: {3}" -f (Test-Path $HostExe), (Test-Path (Join-Path $MozcDir 'cssgsg_mozc.dll')), $startup, $running)
            }
        }
    }
    catch { Write-Host "오류: $_"; $code = 1 }
    exit $code
}

# ---- 이 셸 ------------------------------------------------------------------------------------------
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

# WMI가 띄운 프로세스는 이 셸의 패키지 가상화 밖에서 돈다(사용자 권한, 창 없음). 끝날 때까지 기다리고 기록을 보인다.
function Invoke-Outside([string]$what) {
    $log = Join-Path $Repo 'build\tip-dev-outside.log'
    New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
    Remove-Item $log -ErrorAction SilentlyContinue
    $command = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" $what -Outside -Log `"$log`""
    # 콘솔 창을 숨긴다(윈도우 11 26H2부터 콘솔이 Windows Terminal 창으로 떠서 포커스를 가져갔다).
    $startup = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{ ShowWindow = [uint16]0 }
    $r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = $command; ProcessStartupInformation = $startup }
    if ($r.ReturnValue -ne 0) { throw "WMI로 프로세스를 띄우지 못함 ($($r.ReturnValue))" }
    $deadline = (Get-Date).AddSeconds(60)
    while ((Get-Process -Id $r.ProcessId -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
    if (Test-Path $log) { Get-Content $log -Encoding UTF8 | Where-Object { $_ } | ForEach-Object { "   $_" } }
}

switch ($Action) {
    'install' {
        Write-Step '릴리스 빌드'
        Push-Location $Repo
        # cargo는 진행 상황을 표준 오류로 낸다. Windows PowerShell 5.1은 그걸 오류로 바꾸니(출력을 받아 갈 때) 잠깐 푼다.
        $ErrorActionPreference = 'Continue'
        # 호스트는 따로 빌드한다: 같이 빌드하면 코어 기능이 합쳐져 Mozc 변환기 코드가 입력기 DLL에도 들어간다.
        try {
            cargo build --release -p cssgsg-tip 2>&1 | ForEach-Object { "$_" }
            $built = $LASTEXITCODE -eq 0
            if ($built) { cargo build --release -p cssgsg-host 2>&1 | ForEach-Object { "$_" }; $built = $LASTEXITCODE -eq 0 }
        }
        finally { Pop-Location; $ErrorActionPreference = 'Stop' }
        if (-not $built) { throw 'cargo build 실패' }
        Invoke-Elevated 'install' (Join-Path $Repo 'build\cargo\release\cssgsg_tip.dll')
        Write-Step '내 입력 목록에 추가'
        Invoke-Outside 'enable'
        Invoke-Outside 'status'
        # 지난 빌드의 옛 판(하루 넘게 안 쓴 것)을 지운다. 정리가 실패해도 설치는 그대로다.
        try { & (Join-Path $PSScriptRoot 'prune-build.ps1') } catch { Write-Warning "prune-build: $_" }
    }
    'uninstall' {
        Write-Step '내 입력 목록에서 빼고 사용자 흔적 지우기'
        Invoke-Outside 'disable'
        Invoke-Elevated 'uninstall' $null
        Invoke-Outside 'status'
    }
    default { Invoke-Outside $Action }
}
