# ==============================================================================
# NESS Relay v2.0.0 - JSON Exporter
# ==============================================================================
# Exporta los datos recolectados a archivos JSON locales.
# ==============================================================================

import json
import logging
from datetime import datetime
from pathlib import Path
from typing import Any, Optional

from core.config import OUTPUT_DIR

logger = logging.getLogger("ness_relay")


def export_to_json(
    data: Any,
    filename: Optional[Path] = None,
    output_dir: Optional[Path] = None,
) -> Optional[str]:
    """
    Exporta datos a un archivo JSON.
    
    Args:
        data: Datos a serializar (dict, list, etc.).
        filename: Ruta completa del archivo de salida.
                  Si es None, genera un nombre con timestamp.
        output_dir: Directorio de salida (default: OUTPUT_DIR de config).
        
    Returns:
        Ruta del archivo creado como string, o None si falló.
    """
    if output_dir is None:
        output_dir = OUTPUT_DIR
    
    if filename is None:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        filename = output_dir / f"relay_monitoring_{timestamp}.json"
    
    try:
        # Asegurar que el directorio existe
        filename = Path(filename)
        filename.parent.mkdir(parents=True, exist_ok=True)
        
        with open(filename, 'w', encoding='utf-8') as f:
            json.dump(data, f, indent=2, ensure_ascii=False, default=str)
        
        file_size = filename.stat().st_size
        logger.info(f"Datos exportados exitosamente a {filename} ({file_size} bytes)")
        return str(filename)
        
    except Exception as e:
        logger.exception(f"Error al exportar datos a JSON: {e}")
        return None
