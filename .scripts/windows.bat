@echo off
setlocal enabledelayedexpansion

if exist .env (
    for /f "tokens=*" %%a in (.env) do set %%a
)

echo Building Scope...
cargo build --release

echo Creating installer directory structure...
mkdir installer\bin

echo Copying files...
copy target\release\scope.exe installer\bin\
copy .github\scope-round-200.png installer\scope.ico

echo Creating WiX files...
(
echo ^<?xml version='1.0' encoding='windows-1252'?^>
echo ^<Wix xmlns='http://schemas.microsoft.com/wix/2006/wi'^>
echo   ^<Product Name='Scope' Manufacturer='Scope' Id='*' UpgradeCode='12345678-1234-1234-1234-111111111111'
echo     Language='1033' Codepage='1252' Version='1.0.0'^>
echo     ^<Package Id='*' Keywords='Installer' Description='Scope Installer'
echo       Comments='Scope is a development tool' Manufacturer='Scope'
echo       InstallerVersion='100' Languages='1033' Compressed='yes' SummaryCodepage='1252' /^>
echo     ^<Media Id='1' Cabinet='Scope.cab' EmbedCab='yes' /^>
echo     ^<Directory Id='TARGETDIR' Name='SourceDir'^>
echo       ^<Directory Id='ProgramFilesFolder' Name='PFiles'^>
echo         ^<Directory Id='INSTALLDIR' Name='Scope'^>
echo           ^<Component Id='MainExecutable' Guid='12345678-1234-1234-1234-222222222222'^>
echo             ^<File Id='ScopeEXE' Name='scope.exe' DiskId='1' Source='installer\bin\scope.exe' KeyPath='yes'^>
echo               ^<Shortcut Id='startmenuScope' Directory='ProgramMenuDir' Name='Scope' WorkingDirectory='INSTALLDIR' Icon='Scope.exe' IconIndex='0' Advertise='yes' /^>
echo             ^</File^>
echo           ^</Component^>
echo         ^</Directory^>
echo       ^</Directory^>
echo       ^<Directory Id='ProgramMenuFolder' Name='Programs'^>
echo         ^<Directory Id='ProgramMenuDir' Name='Scope'^>
echo           ^<Component Id='ProgramMenuDir' Guid='12345678-1234-1234-1234-333333333333'^>
echo             ^<RemoveFolder Id='ProgramMenuDir' On='uninstall' /^>
echo             ^<RegistryValue Root='HKCU' Key='Software\[Manufacturer]\[ProductName]' Type='string' Value='' KeyPath='yes' /^>
echo           ^</Component^>
echo         ^</Directory^>
echo       ^</Directory^>
echo     ^</Directory^>
echo     ^<Feature Id='Complete' Level='1'^>
echo       ^<ComponentRef Id='MainExecutable' /^>
echo       ^<ComponentRef Id='ProgramMenuDir' /^>
echo     ^</Feature^>
echo     ^<Icon Id='Scope.exe' SourceFile='installer\scope.ico' /^>
echo   ^</Product^>
echo ^</Wix^>
) > scope.wxs

echo Building MSI...
candle scope.wxs
light -ext WixUIExtension scope.wixobj

echo Cleaning up build files...
rmdir /s /q installer
del scope.wxs
del scope.wixobj
del scope.wixpdb

echo Build complete! MSI installer is ready: scope.msi
