# ==============================================================================
# NESS Relay v2.0.0 - Server Sender
# ==============================================================================
# Envía los datos recolectados al servidor NESS via API REST.
# ==============================================================================

import logging
from typing import Any, Dict
from urllib.parse import urlparse

import requests

from core.config import NESS_API_TOKEN, NESS_SERVER_URL

logger = logging.getLogger("ness_relay")


def send_data_to_server(
    data: Dict[str, Any],
    server_url: str = "",
    api_token: str = "",
    timeout: int = 30,
) -> bool:
    """
    Envía los datos recolectados al servidor NESS via POST.
    
    Args:
        data: Diccionario con los datos a enviar.
        server_url: URL del endpoint API. Si vacío, usa NESS_SERVER_URL.
        api_token: Token de autenticación. Si vacío, usa NESS_API_TOKEN.
        timeout: Timeout en segundos para la solicitud HTTP.
        
    Returns:
        True si el envío fue exitoso (HTTP 200), False en caso contrario.
    """
    url = server_url or NESS_SERVER_URL
    token = api_token or NESS_API_TOKEN
    
    if not token:
        logger.warning("NESS_API_TOKEN no configurado. No se enviarán datos al servidor.")
        return False
    
    headers = {
        'Authorization': f'Token {token}',
        'Content-Type': 'application/json'
    }
    
    try:
        # Extraer hostname para logs (sin exponer URL completa)
        parsed = urlparse(url)
        server_host = parsed.hostname or parsed.netloc.split(':')[0]
        logger.info(f"Enviando datos a {server_host}")
        
        response = requests.post(url, json=data, headers=headers, timeout=timeout)
        
        if response.status_code == 200:
            logger.info("Datos enviados exitosamente al servidor.")
            return True
        else:
            logger.error(f"Error al enviar datos: {response.status_code} - {response.text}")
            return False
            
    except requests.exceptions.Timeout:
        logger.error(f"Timeout al enviar datos al servidor ({timeout}s)")
        return False
    except requests.exceptions.ConnectionError:
        logger.error("No se pudo conectar al servidor NESS")
        return False
    except Exception as e:
        logger.error(f"Excepción al enviar datos al servidor: {e}")
        return False
