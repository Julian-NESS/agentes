# ==============================================================================
# NESS Relay v2.0.0 - Crypto Initialization
# ==============================================================================
# Inicialización del backend criptográfico para PyInstaller frozen executables.
# DEBE ejecutarse ANTES de cualquier import de pysnmp para que SNMPv3 funcione.
# ==============================================================================

import importlib
import sys


def init_crypto_backend() -> bool:
    """
    Inicializa Cryptodome como backend criptográfico para pysnmp.
    
    En binarios compilados con PyInstaller, los módulos de Cryptodome
    no se descubren automáticamente. Esta función los carga explícitamente
    para que pysnmpcrypto pueda usarlos para SNMPv3 (AES/DES).
    
    Returns:
        True si la inicialización fue exitosa, False en caso contrario.
    """
    try:
        # Intentar importar Cryptodome directamente
        from Cryptodome.Cipher import AES, DES  # noqa: F401
        from Cryptodome.Hash import MD5, SHA1, HMAC  # noqa: F401
        
        # Registrar en sys.modules para que pysnmpcrypto los encuentre
        crypto_modules = [
            'Cryptodome', 'Cryptodome.Cipher', 'Cryptodome.Cipher.AES',
            'Cryptodome.Cipher.DES', 'Cryptodome.Hash', 'Cryptodome.Hash.MD5',
            'Cryptodome.Hash.SHA1', 'Cryptodome.Hash.HMAC'
        ]
        
        for mod_name in crypto_modules:
            if mod_name not in sys.modules:
                try:
                    sys.modules[mod_name] = importlib.import_module(mod_name)
                except ImportError:
                    pass
        
        # También registrar como Crypto (alias usado por algunos paquetes)
        if 'Crypto' not in sys.modules and 'Cryptodome' in sys.modules:
            sys.modules['Crypto'] = sys.modules['Cryptodome']
        
        return True
    except ImportError:
        # Cryptodome no disponible - SNMPv3 con cifrado no funcionará
        return False


def suppress_warnings() -> None:
    """Suprime warnings que no son relevantes en producción."""
    import warnings
    warnings.filterwarnings('ignore', category=DeprecationWarning)
    warnings.filterwarnings('ignore', message='.*cryptography.*')
    warnings.filterwarnings('ignore', message='.*Blowfish.*')


def setup_unbuffered_output() -> None:
    """Configura stdout/stderr sin buffer para logs en tiempo real."""
    import io
    import os
    
    if hasattr(sys.stdout, 'buffer'):
        sys.stdout = io.TextIOWrapper(
            open(sys.stdout.fileno(), 'wb', 0),
            encoding='utf-8',
            errors='replace',
            write_through=True
        )
    if hasattr(sys.stderr, 'buffer'):
        sys.stderr = io.TextIOWrapper(
            open(sys.stderr.fileno(), 'wb', 0),
            encoding='utf-8',
            errors='replace',
            write_through=True
        )
    
    os.environ['PYTHONUNBUFFERED'] = '1'
