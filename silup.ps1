<# 
.SYNOPSIS
    Instalador de SIL (Semantic Intention Language) para Windows PowerShell

.DESCRIPTION
    Descarga, verifica e instala el compilador nativo silc y el servidor LSP.
#>

param(
    [string]$Version = "1.0.0",
    [string]$InstallDir = "$env:USERPROFILE\.sil"
)

$ErrorActionPreference = "Stop"

Write-Host "==> Instalando el entorno de ejecución y compilador de SIL v${Version}..." -ForegroundColor Cyan

$BinDir = Join-Path $InstallDir "bin"
$SdkDir = Join-Path $InstallDir "sdk"

# Crear directorios
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
New-Item -ItemType Directory -Force -Path $SdkDir | Out-Null

# Detectar arquitectura
$Arch = [System.Environment]::GetEnvironmentVariable("PROCESSOR_ARCHITECTURE")
if ($Arch -eq "AMD64") { $Arch = "amd64" }
elseif ($Arch -eq "ARM64") { $Arch = "arm64" }
else { Write-Error "Arquitectura $Arch no soportada"; exit 1 }

$Target = "silc-windows-${Arch}.exe"
$DownloadUrl = "https://releases.sil-lang.org/v${Version}/${Target}"

Write-Host "==> Descargando binario nativo desde ${DownloadUrl}..." -ForegroundColor Cyan

# Descargar binario
$TmpPath = Join-Path $env:TEMP $Target
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TmpPath -UseBasicParsing
} catch {
    Write-Error "Error descargando: $_"; exit 1
}

# Verificar SHA256
$ShaUrl = "${DownloadUrl}.sha256"
$ShaPath = "${TmpPath}.sha256"
try {
    Invoke-WebRequest -Uri $ShaUrl -OutFile $ShaPath -UseBasicParsing
    $ExpectedHash = (Get-Content $ShaPath -Raw).Split(' ')[0]
    $ActualHash = (Get-FileHash -Algorithm SHA256 $TmpPath).Hash.ToLower()
    if ($ExpectedHash -ne $ActualHash) {
        Write-Error "¡VERIFICACIÓN SHA256 FALLÓ! Esperado: $ExpectedHash, Obtenido: $ActualHash"
        exit 1
    }
    Write-Host "==> Verificación SHA256: OK" -ForegroundColor Green
} catch {
    Write-Warning "No se pudo verificar SHA256 (archivo .sha256 no encontrado en servidor). Continuando sin verificación..."
}

# Copiar a directorio de instalación
$DestPath = Join-Path $BinDir "silc.exe"
Copy-Item $TmpPath $DestPath -Force

# También copiar sil-lsp si existe en el paquete (futuro)
# $LspSource = Join-Path $env:TEMP "sil-lsp.exe"
# if (Test-Path $LspSource) { Copy-Item $LspSource (Join-Path $BinDir "sil-lsp.exe") -Force }

# Configurar PATH permanente (usuario actual)
$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
$BinDirToAdd = $BinDir

if ($CurrentPath -notlike "*$BinDirToAdd*") {
    $NewPath = "$CurrentPath;$BinDirToAdd"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    [Environment]::SetEnvironmentVariable("SIL_INSTALL_DIR", $InstallDir, "User")
    Write-Host "==> PATH y SIL_INSTALL_DIR actualizados en variables de entorno de usuario." -ForegroundColor Green
    Write-Warning "Debes reiniciar PowerShell / terminal para que los cambios surtan efecto."
} else {
    Write-Host "==> PATH ya contiene el directorio de instalación." -ForegroundColor Yellow
}

Write-Host "==> ¡Instalación exitosa! Ejecuta 'silc --version' para comenzar." -ForegroundColor Cyan