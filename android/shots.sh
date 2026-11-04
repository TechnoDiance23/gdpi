#!/bin/sh
# Emülatörde GDPI'ın ekran görüntülerini alır (CI'daki android-emulator-runner içinden çalışır).
set -e
P=io.github.technodiance23.gdpi
OUT=build/shots
mkdir -p $OUT
# yavaş emülatörde "Pixel Launcher yanıt vermiyor" penceresi çıkmasın
adb shell settings put global hide_error_dialogs 1
adb shell settings put secure anr_show_background 0
sleep 20
adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS || true
adb install -r app/build/outputs/apk/release/app-release.apk
adb shell appops set $P ACTIVATE_VPN allow          # VPN izin penceresini önceden onayla
adb shell pm grant $P android.permission.POST_NOTIFICATIONS || true
adb shell settings put global sysui_demo_allowed 1
adb shell am broadcast -a com.android.systemui.demo -e command enter
adb shell am broadcast -a com.android.systemui.demo -e command clock -e hhmm 1200
adb shell am broadcast -a com.android.systemui.demo -e command notifications -e visible false
adb shell am start -W -n $P/.MainActivity
sleep 8

tap() {
  adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null || true
  adb shell uiautomator dump /sdcard/u.xml >/dev/null
  adb pull /sdcard/u.xml $OUT/u.xml >/dev/null
  xy=$(python3 - "$1" $OUT/u.xml <<'PY'
import re, sys
want, xml = sys.argv[1], open(sys.argv[2], encoding="utf-8").read()
for n in re.finditer(r'<node [^>]*>', xml):
    t = n.group(0)
    if f'text="{want}"' in t or f'content-desc="{want}"' in t:
        a, b, c, d = map(int, re.search(r'bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"', t).groups())
        print((a + c) // 2, (b + d) // 2); break
PY
)
  [ -n "$xy" ] || { echo "bulunamadı: $1"; exit 1; }
  adb shell input tap $xy
}
shot() { adb shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null || true; sleep 1; adb exec-out screencap -p > $OUT/$1.png; }

shot kapali
tap "Aç / kapat"; sleep 6
adb shell dumpsys connectivity | grep -i -m1 "VPN" || true
shot acik
tap "Siteler"; sleep 2; shot siteler
tap "Ayarlar"; sleep 2; shot ayarlar
ls -la $OUT
