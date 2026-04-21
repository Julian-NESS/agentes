# PyInstaller hook for pysnmp
# Asegura que todos los módulos necesarios para SNMPv3 se incluyan

from PyInstaller.utils.hooks import collect_submodules, collect_data_files

# Recolectar todos los submódulos de pysnmp
hiddenimports = collect_submodules('pysnmp')

# Añadir módulos específicos de autenticación y cifrado
hiddenimports += [
    'pysnmp.hlapi.v3arch.auth',
    'pysnmp.hlapi.v3arch.lcd',
    'pysnmp.proto.secmod.rfc3414',
    'pysnmp.proto.secmod.rfc3414.auth',
    'pysnmp.proto.secmod.rfc3414.priv',
    'pysnmp.proto.secmod.rfc3826',
    'pysnmp.proto.secmod.rfc3826.priv',
    'pysnmp.proto.secmod.rfc7860',
    'pysnmp.proto.secmod.rfc7860.auth',
]

# Recolectar archivos de datos (MIBs, etc.)
datas = collect_data_files('pysnmp')
