# ==============================================================================
# package.ps1
# Скрипт сборки и упаковки релиза csqtt-win для Windows x64
# Windows PowerShell 5.1 совместимый скрипт
# ==============================================================================

[CmdletBinding()]
param(
    [string]$Version,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

$PSScriptDir = $PSScriptRoot
if (-not $PSScriptDir) {
    $PSScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
}

# Корневой каталог репозитория (D:\Claude main\work\csqtt-src)
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptDir '..\..'))
$RustClientDir = Join-Path $RepoRoot 'rust-client'
$WindowsDir = Join-Path $RepoRoot 'windows'
$DistDir = Join-Path $RepoRoot 'dist'
$WintunCacheDir = Join-Path $PSScriptDir 'cache\wintun'

# 1. Определение версии по умолчанию из windows\csqtt-svc\Cargo.toml
if ([string]::IsNullOrWhiteSpace($Version)) {
    $svcCargoToml = Join-Path $WindowsDir 'csqtt-svc\Cargo.toml'
    if (-not (Test-Path $svcCargoToml)) {
        throw "Не найден файл $svcCargoToml для чтения версии по умолчанию"
    }
    $tomlLines = Get-Content -Path $svcCargoToml
    foreach ($line in $tomlLines) {
        if ($line -match '^\s*version\s*=\s*["'']([^"'']+)["'']') {
            $Version = $matches[1]
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($Version)) {
        throw "Не удалось определить версию из файла $svcCargoToml"
    }
    Write-Host "Версия приложения определена из csqtt-svc\Cargo.toml: $Version"
} else {
    Write-Host "Используется переданная версия: $Version"
}

# 2. Определение реального имени исполняемого файла из rust-client\Cargo.toml
$rustClientCargoToml = Join-Path $RustClientDir 'Cargo.toml'
$clientBinName = 'client'
if (Test-Path $rustClientCargoToml) {
    $clientTomlLines = Get-Content -Path $rustClientCargoToml
    $inBin = $false
    $foundBin = $false
    $pkgName = ''
    foreach ($line in $clientTomlLines) {
        if ($line -match '^\s*\[\[bin\]\]') {
            $inBin = $true
            continue
        }
        if ($inBin -and $line -match '^\s*\[') {
            $inBin = $false
        }
        if ($inBin -and $line -match '^\s*name\s*=\s*["'']([^"'']+)["'']') {
            $clientBinName = $matches[1]
            $foundBin = $true
            break
        }
        if (-not $foundBin -and $line -match '^\s*name\s*=\s*["'']([^"'']+)["'']' -and [string]::IsNullOrEmpty($pkgName)) {
            $pkgName = $matches[1]
        }
    }
    if (-not $foundBin -and -not [string]::IsNullOrEmpty($pkgName)) {
        $clientBinName = $pkgName
    }
}
Write-Host "Имя бинарного файла rust-client: $clientBinName.exe"

# 3. Сборка (если не указан флаг -SkipBuild)
if (-not $SkipBuild) {
    Write-Host "Запуск сборки rust-client (cargo build --release)..."
    Push-Location $RustClientDir
    try {
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            throw "Ошибка при сборке rust-client (код выхода: $LASTEXITCODE)"
        }
    } finally {
        Pop-Location
    }

    Write-Host "Запуск сборки csqtt-svc и csqtt-cli (cargo build --release -p csqtt-svc -p csqtt-cli)..."
    Push-Location $WindowsDir
    try {
        cargo build --release -p csqtt-svc -p csqtt-cli
        if ($LASTEXITCODE -ne 0) {
            throw "Ошибка при сборке csqtt-svc и csqtt-cli (код выхода: $LASTEXITCODE)"
        }
    } finally {
        Pop-Location
    }
} else {
    Write-Host "Сборка пропущена (-SkipBuild)."
}

# 4. Проверка и подготовка драйвера Wintun
$wintunDll = Join-Path $WintunCacheDir 'wintun.dll'
$wintunLicense = Join-Path $WintunCacheDir 'LICENSE.txt'

if (-not (Test-Path $wintunDll) -or -not (Test-Path $wintunLicense)) {
    Write-Host "Wintun не найден в кэше. Запуск fetch-wintun.ps1..."
    $fetchScript = Join-Path $PSScriptDir 'fetch-wintun.ps1'
    if (Test-Path $fetchScript) {
        & $fetchScript
    } else {
        throw "Не найден скрипт загрузки Wintun: $fetchScript"
    }

    if (-not (Test-Path $wintunDll) -or -not (Test-Path $wintunLicense)) {
        throw "Не удалось подготовить файлы Wintun в $WintunCacheDir"
    }
}

# 5. Поиск скомпилированных исполняемых файлов
$builtClientExe = Join-Path $RustClientDir "target\release\$clientBinName.exe"
if (-not (Test-Path $builtClientExe)) {
    $alt = Join-Path $RepoRoot "target\release\$clientBinName.exe"
    if (Test-Path $alt) {
        $builtClientExe = $alt
    } else {
        throw "Не найден исполняемый файл rust-client ($clientBinName.exe): проверены $builtClientExe и $alt"
    }
}

$builtSvcExe = Join-Path $WindowsDir 'target\release\csqtt-svc.exe'
if (-not (Test-Path $builtSvcExe)) {
    $alt = Join-Path $RepoRoot 'target\release\csqtt-svc.exe'
    if (Test-Path $alt) {
        $builtSvcExe = $alt
    } else {
        throw "Не найден исполняемый файл csqtt-svc.exe: проверены $builtSvcExe и $alt"
    }
}

$builtCliExe = Join-Path $WindowsDir 'target\release\csqtt.exe'
if (-not (Test-Path $builtCliExe)) {
    $alt = Join-Path $RepoRoot 'target\release\csqtt.exe'
    if (Test-Path $alt) {
        $builtCliExe = $alt
    } else {
        throw "Не найден исполняемый файл csqtt.exe: проверены $builtCliExe и $alt"
    }
}

# 6. Формирование каталога дистрибутива: dist\csqtt-win-<ver>-windows-x64\
$DistFolderName = "csqtt-win-$Version-windows-x64"
$DistVerDir = Join-Path $DistDir $DistFolderName

if (-not (Test-Path $DistDir)) {
    $null = New-Item -ItemType Directory -Force -Path $DistDir
}

if (Test-Path $DistVerDir) {
    Remove-Item -Recurse -Force -Path $DistVerDir
}
$null = New-Item -ItemType Directory -Force -Path $DistVerDir

Write-Host "Копирование файлов дистрибутива в $DistVerDir..."

# csqtt-svc.exe
Copy-Item -Path $builtSvcExe -Destination (Join-Path $DistVerDir 'csqtt-svc.exe') -Force

# csqtt.exe
Copy-Item -Path $builtCliExe -Destination (Join-Path $DistVerDir 'csqtt.exe') -Force

# client.exe (с переименованием реального имени бинарника в client.exe)
Copy-Item -Path $builtClientExe -Destination (Join-Path $DistVerDir 'client.exe') -Force

# wintun.dll (+ wintun-LICENSE.txt)
Copy-Item -Path $wintunDll -Destination (Join-Path $DistVerDir 'wintun.dll') -Force
Copy-Item -Path $wintunLicense -Destination (Join-Path $DistVerDir 'wintun-LICENSE.txt') -Force

# domains.txt (from D:\Claude main\work\csqtt-src\domains.txt)
$domainsPath = Join-Path $RepoRoot 'domains.txt'
if (Test-Path $domainsPath) {
    Copy-Item -Path $domainsPath -Destination (Join-Path $DistVerDir 'domains.txt') -Force
} else {
    Write-Warning "Файл domains.txt ($domainsPath) не найден."
}

# README-windows.md (D:\Claude main\work\csqtt-src\windows\README-windows.md, пишется другим воркером)
$readmePath = Join-Path $WindowsDir 'README-windows.md'
if (Test-Path $readmePath) {
    Copy-Item -Path $readmePath -Destination (Join-Path $DistVerDir 'README-windows.md') -Force
} else {
    Write-Warning "Файл README-windows.md ($readmePath) не найден (ожидается создание другим воркером)."
}

# LICENSE (D:\Claude main\work\csqtt-src\LICENSE)
$licensePath = Join-Path $RepoRoot 'LICENSE'
if (Test-Path $licensePath) {
    Copy-Item -Path $licensePath -Destination (Join-Path $DistVerDir 'LICENSE') -Force
} else {
    Write-Warning "Файл LICENSE ($licensePath) не найден."
}

# 7. Архивация в dist\csqtt-win-<ver>-windows-x64.zip и запись SHA256
$zipPath = Join-Path $DistDir "$DistFolderName.zip"
if (Test-Path $zipPath) {
    Remove-Item -Force -Path $zipPath
}

Write-Host "Создание архива $zipPath..."
Compress-Archive -Path $DistVerDir -DestinationPath $zipPath -Force

Write-Host "Вычисление SHA256 архива..."
$zipHash = (Get-FileHash -Path $zipPath -Algorithm SHA256).Hash
Write-Host "SHA256 архива: $zipHash"

$sha256File = Join-Path $DistDir "$DistFolderName.zip.sha256"
$sha256FileAlt = Join-Path $DistDir "$DistFolderName.sha256"
$shaContent = "$zipHash  $DistFolderName.zip"
Set-Content -Path $sha256File -Value $shaContent -Encoding ASCII
Set-Content -Path $sha256FileAlt -Value $shaContent -Encoding ASCII
Write-Host "Контрольная сумма SHA256 сохранена в $sha256File"

# 8. Компиляция инсталлятора через Inno Setup (ISCC.exe), если он найден
$isccExe = $null

$cmdIscc = Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue
if ($cmdIscc) {
    $isccExe = $cmdIscc.Source
} else {
    $pf86 = [System.Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if (-not [string]::IsNullOrEmpty($pf86)) {
        $candidate = Join-Path $pf86 'Inno Setup 6\ISCC.exe'
        if (Test-Path $candidate) {
            $isccExe = $candidate
        }
    }
    if (-not $isccExe) {
        $candidateToolchain = 'D:\Claude main\toolchains\innosetup\ISCC.exe'
        if (Test-Path $candidateToolchain) {
            $isccExe = $candidateToolchain
        }
    }
}

$issPath = Join-Path $WindowsDir 'installer\csqtt-win.iss'

if ($isccExe) {
    if (Test-Path $issPath) {
        Write-Host "Запуск компилятора Inno Setup ($isccExe)..."
        & $isccExe "/DAppVersion=$Version" "/DSourceDir=$DistVerDir" "/O$DistDir" $issPath
        if ($LASTEXITCODE -ne 0) {
            throw "Ошибка компиляции инсталлятора Inno Setup (код выхода: $LASTEXITCODE)"
        }
        Write-Host "Инсталлятор Inno Setup успешно создан в $DistDir"
    } else {
        Write-Host "Файл скрипта инсталлятора не найден ($issPath). Создание инсталлятора пропущено."
    }
} else {
    Write-Host "ISCC.exe не найден. Создание инсталлятора пропущено."
}

Write-Host "Сборка пакета csqtt-win $Version успешно завершена."
