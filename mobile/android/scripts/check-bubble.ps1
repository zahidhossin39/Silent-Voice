# Checks that the app tells a running bubble from one the phone has stopped.
# Needs a phone or emulator on adb with the app installed and set up.
#   1. bubble on
#   2. app process killed (what Xiaomi does on a swipe in Recents)
#   3. switched off and on again
# After each step the app's answer must match whether the system has the
# service bound. Exits 1 on the first mismatch. Pick a device with $env:ANDROID_SERIAL.
$ErrorActionPreference = "Stop"
$adb = "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
$pkg = "app.silentvoice.mobile"
$svc = "$pkg/$pkg.BubbleService"

function Toggle {
    & $adb shell settings delete secure enabled_accessibility_services | Out-Null
    Start-Sleep 1
    & $adb shell settings put secure enabled_accessibility_services $svc
    & $adb shell settings put secure accessibility_enabled 1
    Start-Sleep 3
}
function Report {
    & $adb logcat -c
    # 0x20000000 = SINGLE_TOP. Without it an already-open app only comes to
    # the front (extras don't make an intent "different") and never sees it.
    & $adb shell am start -f 0x20000000 -n "$pkg/.MainActivity" --ez selftest true | Out-Null
    Start-Sleep 7
    $line = & $adb logcat -d -s SVKeepalive | Select-String "bubble="
    if (-not $line) { throw "no SVKeepalive line in logcat" }
    ([string]$line) -replace '.*bubble=(\w+).*', '$1'
}
# The app must agree with the system: bound -> On, switched on but unbound -> Stopped.
function Expect($step) {
    $got = Report
    $bound = (& $adb shell "dumpsys accessibility | grep -c 'label=Silent Voice'").Trim()
    $want = if ($bound -eq "1") { "On" } else { "Stopped" }
    "{0,-22} app says {1,-8} system bound={2}" -f $step, $got, $bound
    if ($got -ne $want) { "FAIL: wanted $want"; exit 1 }
}

Toggle
Expect "1. on"
# Stock Android restarts the killed service (bound=1); Xiaomi without
# Autostart leaves it dead (bound=0). Either way the app must say which.
& $adb shell am crash $pkg
Start-Sleep 5
# Repeated forced crashes raise a "keeps stopping" dialog that blocks the next launch.
& $adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS | Out-Null
Expect "2. killed"
Toggle
Expect "3. switched again"
"PASS"
