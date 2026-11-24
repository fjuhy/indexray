$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    $release = Join-Path $PSScriptRoot 'release'
    $portable = Join-Path $release 'IndexRay-portable-x64'
    $dictionary = Join-Path $PSScriptRoot 'target/ko-dic'
    $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
    $env:LINDERA_BUILD_DICTIONARY_CACHE_DIR = Join-Path $target 'dictionary-cache'
    if (!(Test-Path "$dictionary/metadata.json")) {
        node native/prepare-ko-dic.mjs --build --require
        if ($LASTEXITCODE) { throw 'Korean dictionary build failed' }
    }
    cargo build --locked --release --bins
    if ($LASTEXITCODE) { throw 'IndexRay build failed' }
    if (Test-Path $portable) { Remove-Item $portable -Recurse -Force }
    New-Item -ItemType Directory -Force $portable | Out-Null
    Copy-Item (Join-Path $target 'release/IndexRay.exe') "$portable/IndexRay.exe"
    Copy-Item (Join-Path $target 'release/indexray-cli.exe') "$portable/indexray-cli.exe"
    Copy-Item $dictionary "$portable/ko-dic" -Recurse
    Copy-Item 'LICENSE-MIT' "$portable/LICENSE-MIT.txt"
    Copy-Item 'Guide.md' "$portable/Guide.md"
    python native/third-party-notices.py "$portable/THIRD-PARTY-NOTICES.txt"
    if ($LASTEXITCODE) { throw 'Third-party notice collection failed' }
    $compiler = Get-Command makensis.exe -ErrorAction SilentlyContinue
    $nsis = if ($compiler) { $compiler.Source } else { "$env:LOCALAPPDATA/tauri/NSIS/Bin/makensis.exe" }
    & $nsis /V2 "$PSScriptRoot/native/installer.nsi"
    if ($LASTEXITCODE) { throw 'Installer build failed' }
    Compress-Archive -Path "$portable/*" -DestinationPath "$release/IndexRay-portable-x64.zip" -Force
    Compress-Archive -LiteralPath 'skills/search-document-library/SKILL.md' -DestinationPath "$release/search-document-library.zip" -Force
    $assets = 'IndexRay-x64-setup.exe', 'IndexRay-portable-x64.zip', 'search-document-library.zip'
    python -c "import hashlib, pathlib, sys; root = pathlib.Path(sys.argv[1]); (root / 'SHA256SUMS.txt').write_text(''.join(hashlib.sha256((root / name).read_bytes()).hexdigest() + '  ' + name + '\n' for name in sys.argv[2:]), encoding='ascii', newline='\n')" $release @assets
    if ($LASTEXITCODE) { throw 'Checksum generation failed' }
} finally {
    Pop-Location
}
