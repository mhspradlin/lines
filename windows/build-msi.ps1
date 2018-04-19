Push-Location (Join-Path $MyInvocation.MyCommand.Path \..\..\)
[Environment]::CurrentDirectory = $PWD

# Make sure we've built the release version of the package
cargo build --release

$manifest = cargo read-manifest --manifest-path Cargo.toml | ConvertFrom-Json
$version = $manifest.version.Split(".")
$env:CFG_VER_MAJOR = $version[0]
$env:CFG_VER_MINOR = $version[1]
$env:CFG_VER_PATCH = $version[2]
$env:NSSM_VERSION = "2.24-101-g897c7ad"
$env:PACKAGE_NAME = "LinesAgent"
$env:PACKAGE_DESCRIPTION = "Sends basic system health metrics to a metrics aggregator"

if (!(Get-ChildItem "target\nssm-$env:NSSM_VERSION").Exists) {
    # Taken from the consul-agent install.ps1 file
    # Download nssm.zip
    Write-Host Downloading NSSM ZIP
    $WebClient = New-Object System.Net.WebClient
    try {
        $WebClient.DownloadFile("http://www.nssm.cc/ci/nssm-$env:NSSM_VERSION.zip", "target\nssm.zip")
    }
    catch [System.Net.WebException] {
        # $_ is set to the ErrorRecord of the exception
        if ($_.Exception.InnerException) {
            Write-Host $_.Exception.InnerException.Message
        }
        else {
            Write-Host $_.Exception.Message
        }
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    function Unzip {
        param([string]$zipfile, [string]$outpath)
        [System.IO.Compression.ZipFile]::ExtractToDirectory($zipfile, $outpath)
    }

    # Unpack nssm.zip
    Unzip "target\nssm.zip" "target\"
    # End copying
}

foreach ($file in Get-ChildItem windows\*.wxs) {
    $out = $($file.Name.Replace(".wxs", ".wixobj"))
    &"$($env:WIX)bin\candle.exe" -nologo -ext WixUtilExtension -out "target\$out" $file
    if ($LASTEXITCODE -ne 0) { exit 1 }
}

&"$($env:WIX)\bin\light.exe" -nologo -ext WixUtilExtension -pdbout "target\installer.pdb" -out "windows\installer.msi" $(Get-ChildItem target\*.wixobj)

Pop-Location
[Environment]::CurrentDirectory = $PWD