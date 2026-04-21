# PyInstaller hook for pysnmpcrypto
# Este hook asegura que todos los módulos de pysnmpcrypto se incluyan
# pysnmpcrypto proporciona la integración entre pysnmp y pycryptodome

from PyInstaller.utils.hooks import collect_submodules, collect_data_files, collect_dynamic_libs

# Recolectar todos los submódulos de pysnmpcrypto
hiddenimports = collect_submodules('pysnmpcrypto')

# Añadir las dependencias explícitas
hiddenimports += [
    'Crypto',
    'Crypto.Cipher',
    'Crypto.Cipher.AES',
    'Crypto.Cipher.DES',
    'Crypto.Cipher.DES3',
    'Crypto.Hash',
    'Crypto.Hash.MD5',
    'Crypto.Hash.SHA256',
    'Crypto.Hash.HMAC',
    'Crypto.Random',
    'Crypto.Util',
]

# Recolectar archivos de datos
datas = collect_data_files('pysnmpcrypto')

# Recolectar librerías dinámicas
binaries = collect_dynamic_libs('pysnmpcrypto')
