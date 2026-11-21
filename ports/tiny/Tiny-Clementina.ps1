[CmdletBinding(DefaultParameterSetName = 'Conversacion')]
param(
    [Parameter(ParameterSetName = 'Conversacion', Position = 0)]
    [Parameter(ParameterSetName = 'Cinta', Position = 0)]
    [ValidateNotNullOrEmpty()]
    [string]$Pregunta,

    [string]$Checkpoint = (Join-Path $env:USERPROFILE 'Downloads\tiny68k\models_retro\tiny_retro_exp3\r3_80x2L2x256.pt'),

    [switch]$Ternario,

    [switch]$Compacto,

    [Parameter(ParameterSetName = 'Conversacion')]
    [Parameter(ParameterSetName = 'Cinta')]
    [ValidateRange(1, 48)]
    [int]$SalidaMaxima = 48,

    [Parameter(ParameterSetName = 'Conversacion')]
    [Parameter(ParameterSetName = 'Cinta')]
    [ValidateScript({ $_ -eq 0 -or ($_ -ge 0.01 -and $_ -le 10) })]
    [double]$Temperatura = 0,

    [Parameter(ParameterSetName = 'Conversacion')]
    [Parameter(ParameterSetName = 'Cinta')]
    [ValidateRange(1, 1023)]
    [int]$Semilla = 1,

    [Parameter(ParameterSetName = 'Conversacion')]
    [string]$CintaSvg,

    [Parameter(ParameterSetName = 'Cinta', Mandatory = $true)]
    [switch]$Preparar,

    [Parameter(ParameterSetName = 'Verificacion', Mandatory = $true)]
    [switch]$Verificar,

    [Parameter(ParameterSetName = 'EnVivo', Mandatory = $true)]
    [switch]$EnVivo
)

$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
if ($Ternario -and $Compacto) { throw 'Elegí -Ternario o -Compacto.' }
if ($EnVivo) {
    $variante = if ($Compacto) { 'compacto' } elseif ($Ternario) { 'ternario' } else { '3bit' }
    python (Join-Path $PSScriptRoot 'en_vivo.py') --variante $variante
    exit $LASTEXITCODE
}
if (-not $PSBoundParameters.ContainsKey('Checkpoint')) {
    if ($Ternario) {
        $Checkpoint = Join-Path $env:USERPROFILE 'Downloads\tiny68k\models_retro\tiny_retro_exp3\rt3_96_wd05.pt'
    }
    if ($Compacto) {
        $Checkpoint = Join-Path $env:USERPROFILE 'Downloads\tiny68k\models_retro\tiny_retro_3bit\r3_80x3L2x224.pt'
    }
}
if (-not (Test-Path -LiteralPath $Checkpoint -PathType Leaf)) {
    throw "No se encontró el checkpoint en $Checkpoint. Indicá la ubicación con -Checkpoint."
}
$Checkpoint = (Resolve-Path -LiteralPath $Checkpoint).Path
$exportador = Join-Path $PSScriptRoot 'exportar.py'
if ($CintaSvg) {
    if (-not $PSBoundParameters.ContainsKey('Pregunta')) {
        throw '-CintaSvg requiere una pregunta.'
    }
    $CintaSvg = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($CintaSvg)
}

Push-Location -LiteralPath $repo
try {
    $hashModelo = (Get-FileHash -LiteralPath $Checkpoint).Hash
    $hashExportador = (Get-FileHash -LiteralPath $exportador).Hash
    $imagen = Join-Path $repo "target\tiny\$hashModelo-$hashExportador.bin"
    if (-not (Test-Path -LiteralPath $imagen -PathType Leaf)) {
        python $exportador $Checkpoint $imagen
        if ($LASTEXITCODE -ne 0) { throw 'Falló la exportación del tambor de TINY.' }
    }

    $firma = [Text.Encoding]::Unicode.GetString([IO.File]::ReadAllBytes($imagen), 0, 12)
    $esTernario = $firma -eq 'TINYT1'
    $esCompacto = $firma -eq 'TINYD1'
    if ($Verificar -and $firma -eq 'TINYC1') {
        cargo build --quiet --release -p mercury-core --example tiny_clementina
        if ($LASTEXITCODE -ne 0) { throw 'Falló la compilación del verificador de TINY.' }
        & (Join-Path $repo 'target\release\examples\tiny_clementina.exe') $imagen --verificar
        if ($LASTEXITCODE -ne 0) { throw 'Falló la verificación de las proyecciones.' }
        return
    }
    $destino = Join-Path $repo 'target\tiny\nativo'
    if ($esTernario) { $destino += '-ternario' }
    if ($esCompacto) { $destino += '-compacto' }
    if ($SalidaMaxima -ne 48) { $destino += "-$SalidaMaxima" }
    $temperaturaTexto = $Temperatura.ToString('R', [Globalization.CultureInfo]::InvariantCulture)
    if ($Temperatura -gt 0) { $destino += "-t$temperaturaTexto-s$Semilla" }
    python (Join-Path $PSScriptRoot 'nativo.py') $imagen $destino --salida-maxima $SalidaMaxima --temperatura $temperaturaTexto --semilla $Semilla
    if ($LASTEXITCODE -ne 0) { throw 'Falló la preparación de los bancos Mercury.' }
    if ($Verificar) {
        cargo build --quiet --release -p mercury-core --example medir_proyecciones
        if ($LASTEXITCODE -ne 0) { throw 'Falló la compilación del verificador de proyecciones.' }
        & (Join-Path $repo 'target\release\examples\medir_proyecciones.exe') $imagen $destino
        if ($LASTEXITCODE -ne 0) { throw 'Falló la verificación de las proyecciones.' }
        return
    }
    cargo build --quiet --release -p mercury-core --example grabar_cinta
    if ($LASTEXITCODE -ne 0) { throw 'Falló la compilación del grabador de cinta.' }
    cargo build --quiet --release -p mercury-cli
    if ($LASTEXITCODE -ne 0) { throw 'Falló la compilación de Clementina.' }
    $bancos = Get-Content -Raw -LiteralPath (Join-Path $destino 'bancos.json') | ConvertFrom-Json
    $sectores = for ($i = 0; $i -lt $bancos.Count; $i += 2) {
        [string]$bancos[$i]
        Join-Path $destino $bancos[$i + 1]
    }
    $grabador = Join-Path $repo 'target\release\examples\grabar_cinta.exe'
    $clementina = Join-Path $repo 'target\release\clementina.exe'
    $base = Join-Path $destino 'tiny'
    $temp = New-TemporaryFile
    $utf8 = [Text.UTF8Encoding]::new($false)
    try {
        $opcionesCinta = @()
        if ($Preparar -and $PSBoundParameters.ContainsKey('Pregunta')) {
            [IO.File]::WriteAllText($temp.FullName, $Pregunta.Trim() + "`n", $utf8)
            $opcionesCinta = @('--cinta', $temp.FullName)
            $base += '-con-pregunta'
        }
        & $grabador (Join-Path $destino 'datos.bin') (Join-Path $destino 'arranque.mer') $base @sectores @opcionesCinta
        if ($LASTEXITCODE -ne 0) { throw 'Falló la grabación de la cinta Ferranti.' }
        if ($Preparar) {
            Write-Output "$base.bin"
            Write-Output "$base.svg"
            Write-Output "$base.vista.svg"
            return
        }

        $opciones = @()
        if ($CintaSvg) { $opciones += @('--svg', $CintaSvg) }
        $interactivo = -not $PSBoundParameters.ContainsKey('Pregunta')
        do {
            if ($interactivo) { $Pregunta = Read-Host 'Pregunta' }
            if ([string]::IsNullOrWhiteSpace($Pregunta)) { break }
            [IO.File]::WriteAllText($temp.FullName, $Pregunta.Trim() + "`n", $utf8)
            & $clementina tele-input "$base.bin" --cinta $temp.FullName @opciones
            if ($LASTEXITCODE -ne 0 -and -not $interactivo) {
                throw 'Clementina terminó con error.'
            }
        } while ($interactivo)
    } finally {
        Remove-Item -LiteralPath $temp.FullName
    }
} finally {
    Pop-Location
}
