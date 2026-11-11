@echo off
rem 真实 cmd 控制台窗口中的验收（任务 6.1）：PATHEXT 解析、重定向字节、管道、退出码。
rem 用法：start "" /wait cmd /c console-check.cmd <dist 目录> <日志文件>
setlocal
set "PATH=%~1;%PATH%"
set "LOG=%~2"
set "WORK=%TEMP%\matcha-6.1-cmd"
if exist "%WORK%" rmdir /s /q "%WORK%"
mkdir "%WORK%"
cd /d "%WORK%"
powershell -NoProfile -Command "[IO.File]::WriteAllText('a.txt', \"one`ntwo`nthree`n\"); [IO.File]::WriteAllText('b.txt', \"one`n2`nthree`nfour`n\"); [IO.File]::WriteAllText('s.txt', \"text-report layout:patch output-to:s.patch a.txt b.txt`r`n\")"
where matcha > "%LOG%"
echo === /report=patch (console) ===
matcha /report=patch a.txt b.txt
echo report to console exit=%ERRORLEVEL% >> "%LOG%"
matcha /report=patch a.txt b.txt > redirected.patch
echo redirect exit=%ERRORLEVEL% >> "%LOG%"
matcha /report=patch /reportfile=direct.patch a.txt b.txt
echo reportfile exit=%ERRORLEVEL% >> "%LOG%"
"%SystemRoot%\System32\fc.exe" /b redirected.patch direct.patch > nul
echo redirect vs reportfile byte-identical (fc errorlevel 0 = same): %ERRORLEVEL% >> "%LOG%"
matcha @s.txt
echo script to file exit=%ERRORLEVEL% >> "%LOG%"
"%SystemRoot%\System32\fc.exe" /b s.patch direct.patch > nul
echo script file vs reportfile byte-identical: %ERRORLEVEL% >> "%LOG%"
matcha /report=patch-unified a.txt b.txt | "%SystemRoot%\System32\findstr.exe" /b "+" >> "%LOG%"
matcha /report=patch nope.txt b.txt 2> missing.err
echo missing file exit=%ERRORLEVEL% >> "%LOG%"
type missing.err >> "%LOG%"
echo ---- console buffer ---- >> "%LOG%"
powershell -NoProfile -Command "$r=$Host.UI.RawUI; $c=$r.GetBufferContents((New-Object Management.Automation.Host.Rectangle 0,0,($r.BufferSize.Width-1),$r.CursorPosition.Y)); for($y=0;$y -le $r.CursorPosition.Y;$y++){ $row=-join (0..($r.BufferSize.Width-1) | %% { $c[$y,$_].Character }); if($row.Trim()){ $row.TrimEnd() } }" >> "%LOG%"
endlocal
