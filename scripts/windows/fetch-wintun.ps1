# ==============================================================================
# fetch-wintun.ps1
# Загрузка и распаковка драйвера Wintun 0.14.1 для csqtt-win
# Windows PowerShell 5.1 совместимый скрипт
# ==============================================================================

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

# ==============================================================================
# Контрольная сумма SHA256 архива wintun-0.14.1.zip.
# Оставлена пустой как плейсхолдер. Если строка пустая, скрипт выводит
# предупреждение и вычисленный хеш, затем продолжает работу.
# ==============================================================================
$WintunSha256 = ''

$WintunUrl = 'https://www.wintun.net/builds/wintun-0.14.1.zip'

$PSScriptDir = $PSScriptRoot
if (-not $PSScriptDir) {
    $PSScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
}

$CacheDir = Join-Path $PSScriptDir 'cache'
$ZipPath = Join-Path $CacheDir 'wintun-0.14.1.zip'
$WintunDestDir = Join-Path $CacheDir 'wintun'

if (-not (Test-Path $CacheDir)) {
    $null = New-Item -ItemType Directory -Force -Path $CacheDir
}

if (-not (Test-Path $WintunDestDir)) {
    $null = New-Item -ItemType Directory -Force -Path $WintunDestDir
}

# 1. Загрузка архива (пропуск, если уже загружен)
if (Test-Path $ZipPath) {
    Write-Host "Архив Wintun уже присутствует в кэше: $ZipPath"
} else {
    Write-Host "Загрузка Wintun с $WintunUrl в $ZipPath..."
    Invoke-WebRequest -Uri $WintunUrl -OutFile $ZipPath -UseBasicParsing
    Write-Host "Загрузка завершена успешно."
}

# 2. Проверка SHA256
Write-Host "Вычисление SHA256 для $ZipPath..."
$computedHash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash

if ([string]::IsNullOrWhiteSpace($WintunSha256)) {
    Write-Warning "Контрольная сумма SHA256 не задана (`$WintunSha256 пуст). Вычисленный хеш: $computedHash"
    Write-Host "Вычисленный SHA256: $computedHash"
} else {
    if (-not ($computedHash.Equals($WintunSha256.Trim(), [System.StringComparison]::OrdinalIgnoreCase))) {
        throw "Несовпадение контрольной суммы SHA256 для $ZipPath. Ожидалось: $WintunSha256, вычислено: $computedHash"
    }
    Write-Host "Проверка контрольной суммы SHA256 успешно пройдена: $computedHash"
}

# 3. Распаковка wintun\bin\amd64\wintun.dll и LICENSE.txt в cache\wintun\
$tempExtractDir = Join-Path $CacheDir 'temp_wintun_extract'
if (Test-Path $tempExtractDir) {
    Remove-Item -Recurse -Force -Path $tempExtractDir
}
$null = New-Item -ItemType Directory -Force -Path $tempExtractDir

try {
    Write-Host "Распаковка архива во временный каталог..."
    Expand-Archive -Path $ZipPath -DestinationPath $tempExtractDir -Force

    # Поиск wintun.dll (amd64)
    $dllSource = Join-Path $tempExtractDir 'wintun\bin\amd64\wintun.dll'
    if (-not (Test-Path $dllSource)) {
        $foundDll = Get-ChildItem -Path $tempExtractDir -Filter 'wintun.dll' -Recurse | Where-Object { $_.FullName -like '*amd64*' } | Select-Object -First 1
        if ($foundDll) {
            $dllSource = $foundDll.FullName
        } else {
            throw "Не удалось найти wintun.dll (amd64) в распакованном архиве"
        }
    }

    # Поиск LICENSE.txt
    $licenseSource = Join-Path $tempExtractDir 'wintun\LICENSE.txt'
    if (-not (Test-Path $licenseSource)) {
        $foundLicense = Get-ChildItem -Path $tempExtractDir -Filter 'LICENSE.txt' -Recurse | Select-Object -First 1
        if ($foundLicense) {
            $licenseSource = $foundLicense.FullName
        } else {
            throw "Не удалось найти LICENSE.txt в распакованном архиве"
        }
    }

    $destDll = Join-Path $WintunDestDir 'wintun.dll'
    $destLicense = Join-Path $WintunDestDir 'LICENSE.txt'

    Copy-Item -Path $dllSource -Destination $destDll -Force
    Copy-Item -Path $licenseSource -Destination $destLicense -Force

    Write-Host "Файлы Wintun успешно извлечены в ${WintunDestDir}:"
    Write-Host "  - $destDll"
    Write-Host "  - $destLicense"
} finally {
    if (Test-Path $tempExtractDir) {
        Remove-Item -Recurse -Force -Path $tempExtractDir
    }
}

Write-Host "fetch-wintun завершён успешно."
