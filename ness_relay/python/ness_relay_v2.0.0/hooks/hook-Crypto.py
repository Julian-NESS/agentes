# PyInstaller hook for pycryptodome (Crypto)
# Este hook asegura que todos los módulos binarios de Crypto se incluyan

from PyInstaller.utils.hooks import collect_submodules, collect_data_files, collect_dynamic_libs

# Recolectar todos los submódulos de Crypto
hiddenimports = collect_submodules('Crypto')

# Recolectar archivos de datos (si los hay)
datas = collect_data_files('Crypto')

# Recolectar librerías dinámicas (.so, .pyd)
binaries = collect_dynamic_libs('Crypto')
