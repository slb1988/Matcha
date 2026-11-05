param(
    [string]$Executable = (Join-Path $PSScriptRoot '..\..\target\release\matcha.exe'),
    [int]$FilesPerSide = 100000,
    [switch]$KeepFixture
)
$ErrorActionPreference='Stop'
$Executable=[IO.Path]::GetFullPath($Executable)
$repo=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$root=Join-Path ([IO.Path]::GetTempPath()) ('matcha-dir-ui-bench-'+[Guid]::NewGuid().ToString('N'))
$out=Join-Path $repo 'target\folder-waveC-bench.log'
$err=Join-Path $repo 'target\folder-waveC-bench.err'
$shot=Join-Path $repo 'docs\worker-reports\assets\folder-waveC-bench.png'
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($shot)) | Out-Null
[IO.Directory]::CreateDirectory($root) | Out-Null
try {
    & cargo build --release --manifest-path (Join-Path $repo 'Cargo.toml') -p bcompare
    if($LASTEXITCODE -ne 0){throw 'GUI build failed'}
    & cargo run --release --manifest-path (Join-Path $repo 'Cargo.toml') -p bc-engine-dir --example dir_ui_fixture -- $root $FilesPerSide
    if($LASTEXITCODE -ne 0){throw 'Fixture generation failed'}
    $left=Join-Path $root 'left';$right=Join-Path $root 'right'
    Write-Output "fixture.root=$root"
    $p=Start-Process -FilePath $Executable -ArgumentList @('--bench-dir',('"'+$left+'"'),('"'+$right+'"')) -RedirectStandardOutput $out -RedirectStandardError $err -PassThru
    $null=$p.Handle
    if(!$p.WaitForExit(240000)){throw "Owned benchmark process $($p.Id) did not finish; inspect it rather than killing unrelated windows."}
    $p.WaitForExit()
    $exit=$p.ExitCode
    Write-Output "benchmark.exit=$exit"
    $log=[IO.File]::ReadAllText($out);Write-Output $log
    $capture=[regex]::Match($log,'(?m)^bench\.dir\.screenshot=(.+)$')
    if($capture.Success){
        Add-Type -AssemblyName System.Drawing
        $bmp=$capture.Groups[1].Value.Trim()
        $image=[Drawing.Image]::FromFile($bmp)
        try{$image.Save($shot,[Drawing.Imaging.ImageFormat]::Png)}finally{$image.Dispose()}
        [IO.File]::Delete($bmp);Write-Output "screenshot=$shot"
    }else{throw 'Native framebuffer screenshot was not produced'}
    if($null -eq $exit -or $exit -ne 0 -or $log -notmatch 'bench\.dir\.pass=true'){throw "Directory benchmark failed; see $out and $err"}
} finally {
    if(!$KeepFixture -and (Test-Path -LiteralPath $root)){[IO.Directory]::Delete($root,$true)}
}
