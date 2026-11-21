# Consulta interactiva de elecciones.mer: arma la cinta con la consulta
# adelante y los datos de cinta.txt atrás, y corre el programa en Clementina.
#
#   .\examples\elecciones\Elecciones-Clementina.ps1                 # pregunta
#   .\examples\elecciones\Elecciones-Clementina.ps1 1962 mapa
#   .\examples\elecciones\Elecciones-Clementina.ps1 1962 ficha 1
#
# Vistas: catalogo, pais, distritos, ficha (con número de distrito),
# diferencia, mapa; o su número, 0 a 5.

param(
    [string]$Eleccion,
    [string]$Vista = 'pais',
    [int]$Distrito = 0
)

$aqui = $PSScriptRoot
$repo = Resolve-Path (Join-Path $aqui '..\..')
$clementina = Join-Path $repo 'target\release\clementina.exe'
$vistas = @{ catalogo = 0; pais = 1; distritos = 2; ficha = 3; diferencia = 4; mapa = 5 }

if (-not (Test-Path $clementina)) {
    Push-Location $repo
    try {
        cargo build --quiet --release -p mercury-cli
        if ($LASTEXITCODE -ne 0) { throw 'Falló la compilación de Clementina.' }
    } finally {
        Pop-Location
    }
}

# Año (o clave del catálogo, como 1948c) → número de elección en la cinta.
$catalogo = @{}
foreach ($linea in Get-Content -LiteralPath (Join-Path $aqui 'catalogo.txt')) {
    $campos = $linea.Trim() -split '\s+'
    if ($campos.Count -ge 2 -and -not $catalogo.ContainsKey($campos[1])) { $catalogo[$campos[1]] = [int]$campos[0] }
}

function Convertir-Consulta([string]$e, [string]$v, [int]$d) {
    if ([string]::IsNullOrWhiteSpace($e) -or $e -eq 'catalogo') { return '0 0 0' }
    $numero = if ($catalogo.ContainsKey($e)) { $catalogo[$e] } elseif ($e -match '^\d+$' -and [int]$e -lt 100) { [int]$e } else { throw "No hay elección $e en la cinta (ver catalogo.txt)." }
    $nv = if ($vistas.ContainsKey($v)) { $vistas[$v] } elseif ($v -match '^[0-5]$') { [int]$v } else { throw "Vista desconocida: $v" }
    if ($nv -eq 3 -and $d -lt 1) { throw 'La ficha necesita un número de distrito (ver la vista distritos).' }
    return "$numero $nv $d"
}

$cinta = Get-Content -Raw -LiteralPath (Join-Path $aqui 'cinta.txt')
$programa = Join-Path $aqui 'elecciones.mer'
$temp = New-TemporaryFile
$utf8 = [Text.UTF8Encoding]::new($false)
try {
    $interactivo = -not $PSBoundParameters.ContainsKey('Eleccion')
    do {
        if ($interactivo) {
            $texto = Read-Host 'Consulta (año vista distrito, Enter para salir)'
            if ([string]::IsNullOrWhiteSpace($texto)) { break }
            $p = $texto.Trim() -split '\s+'
            $Eleccion = $p[0]
            $Vista = if ($p.Count -gt 1) { $p[1] } else { 'pais' }
            $Distrito = if ($p.Count -gt 2) { [int]$p[2] } else { 0 }
        }
        try {
            $consulta = Convertir-Consulta $Eleccion $Vista $Distrito
        } catch {
            Write-Host $_.Exception.Message
            if ($interactivo) { continue } else { exit 1 }
        }
        [IO.File]::WriteAllText($temp.FullName, "$consulta`n$cinta", $utf8)
        & $clementina autocode $programa --cinta $temp.FullName
    } while ($interactivo)
} finally {
    Remove-Item -LiteralPath $temp.FullName
}
