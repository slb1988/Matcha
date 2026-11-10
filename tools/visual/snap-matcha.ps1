param([string]$Scene="",[switch]$All)
$scenes=if($All){@("icon-catalog","chrome-demo","text-basic")}else{@($Scene)}
foreach($s in $scenes){ if(!$s){continue}; $out="target/visual/matcha/$s.png"; New-Item -ItemType Directory -Force (Split-Path $out)|Out-Null; cargo run -p bcompare -- --snapshot=$s --screenshot=$out }
