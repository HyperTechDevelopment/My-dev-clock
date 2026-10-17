@echo off
rem Roda o cargo dentro do ambiente do toolset C++ do Visual Studio, que e o que
rem o rustc precisa para achar o linker do Windows. Aceita qualquer comando do
rem cargo: build.cmd test, build.cmd clippy --all-targets, build.cmd fmt --all.
setlocal
set "VCVARS="

if exist "%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" (
  for /f "usebackq tokens=*" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VCVARS=%%i\VC\Auxiliary\Build\vcvars64.bat"
)

if not defined VCVARS if exist "%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" set "VCVARS=%ProgramFiles(x86)%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if not defined VCVARS if exist "%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" set "VCVARS=%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
if not defined VCVARS if exist "%ProgramFiles%\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" set "VCVARS=%ProgramFiles%\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"

if not defined VCVARS (
  echo Nao encontrei o vcvars64.bat. Instale o Visual Studio com a carga
  echo "Desenvolvimento para desktop com C++".
  exit /b 1
)

rem O caminho do usuario entra nos textos de diagnostico que ficam embutidos no
rem binario, inclusive os das dependencias. Trocar o prefixo por ~ evita publicar
rem o nome de usuario da maquina.
if not defined RUSTFLAGS set "RUSTFLAGS=--remap-path-prefix=%USERPROFILE%=~"

call "%VCVARS%" >nul
cargo %*
