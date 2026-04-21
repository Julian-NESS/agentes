# PyInstaller hook for pycryptodomex (Cryptodome)
# Este hook asegura que todos los módulos binarios de Cryptodome se incluyan

from PyInstaller.utils.hooks import collect_submodules, collect_data_files, collect_dynamic_libs

# Recolectar todos los submódulos de Cryptodome
hiddenimports = collect_submodules('Cryptodome')

# Recolectar archivos de datos (si los hay)
datas = collect_data_files('Cryptodome')

# Recolectar librerías dinámicas (.so, .pyd)
binaries = collect_dynamic_libs('Cryptodome')
