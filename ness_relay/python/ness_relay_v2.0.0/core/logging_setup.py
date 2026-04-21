# ==============================================================================
# NESS Relay v2.0.0 - Logging Setup
# ==============================================================================
# Configuración centralizada del sistema de logging con rotación de archivos.
# ==============================================================================

import logging
import sys
from pathlib import Path
from typing import Optional


def setup_logging(
    log_file: Optional[Path] = None,
    level: int = logging.INFO,
    max_bytes: int = 10 * 1024 * 1024,  # 10 MB
    backup_count: int = 3,
    silent: bool = False,
) -> logging.Logger:
    """
    Configura el logger principal de NESS Relay.
    
    Args:
        log_file: Ruta al archivo de log. Si es None, usa solo consola.
        level: Nivel de logging (default: INFO).
        max_bytes: Tamaño máximo del archivo de log antes de rotar.
        backup_count: Número de archivos de respaldo al rotar.
        silent: Si True, no muestra logs en consola.
        
    Returns:
        Logger configurado.
    """
    logger = logging.getLogger("ness_relay")
    
    # Evitar configurar múltiples veces
    if logger.handlers:
        return logger
    
    logger.setLevel(level)
    
    # Formato de log
    formatter = logging.Formatter(
        fmt='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
        datefmt='%Y-%m-%d %H:%M:%S'
    )
    
    # Handler de archivo con rotación simple
    if log_file is not None:
        try:
            log_file.parent.mkdir(parents=True, exist_ok=True)
            
            # Rotación simple: si el archivo excede max_bytes, renombrar y crear nuevo
            if log_file.exists() and log_file.stat().st_size > max_bytes:
                _rotate_log(log_file, backup_count)
            
            file_handler = logging.FileHandler(
                str(log_file),
                mode='a',
                encoding='utf-8'
            )
            file_handler.setLevel(level)
            file_handler.setFormatter(formatter)
            logger.addHandler(file_handler)
        except Exception as e:
            # Si no se puede crear el archivo de log, continuar sin él
            sys.stderr.write(f"Warning: No se pudo crear archivo de log {log_file}: {e}\n")
    
    # Handler de consola (excepto en modo silencioso)
    if not silent:
        console_handler = logging.StreamHandler(sys.stdout)
        console_handler.setLevel(logging.WARNING)
        console_handler.setFormatter(formatter)
        logger.addHandler(console_handler)
    
    return logger


def _rotate_log(log_file: Path, backup_count: int) -> None:
    """Rota archivos de log manualmente."""
    try:
        # Eliminar el backup más antiguo si excede el conteo
        for i in range(backup_count, 0, -1):
            old_file = log_file.with_suffix(f'.log.{i}')
            if i == backup_count and old_file.exists():
                old_file.unlink()
            elif old_file.exists():
                old_file.rename(log_file.with_suffix(f'.log.{i + 1}'))
        
        # Renombrar el archivo actual
        if log_file.exists():
            log_file.rename(log_file.with_suffix('.log.1'))
    except Exception:
        pass  # Si falla la rotación, simplemente continuar
