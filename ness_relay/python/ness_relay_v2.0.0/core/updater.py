# ==============================================================================
# NESS Relay v2.0.0 - Auto-Updater
# ==============================================================================
# Sistema completo de actualización automática del relay.
# Incluye: verificación de versión, descarga, verificación de hash,
# backup, extracción, reemplazo de ejecutables y limpieza.
# ==============================================================================

import hashlib
import json
import logging
import os
import shutil
import subprocess
import sys
import time
import zipfile
from pathlib import Path
from typing import Any, Dict, Optional

import requests

from core.config import (
    BASE_DIR,
    RELAY_TYPE,
    RELAY_VERSION,
    HOSTING_BASE_URL,
    VERSION_CHECK_URL,
)

logger = logging.getLogger("ness_relay")


# ==============================================================================
# VERIFICACIÓN DE ACTUALIZACIÓN
# ==============================================================================

def verificar_actualizacion() -> Optional[Dict[str, Any]]:
    """
    Verifica si hay una actualización disponible en el servidor.
    
    Returns:
        Dict con info de la nueva versión si hay actualización disponible,
        None si ya está actualizado o hay error.
    """
    try:
        params = {
            'current_version': RELAY_VERSION,
            'relay_type': RELAY_TYPE
        }
        
        logger.info(f"Verificando actualizaciones... (versión actual: {RELAY_VERSION})")
        response = requests.get(VERSION_CHECK_URL, params=params, timeout=15)
        
        if response.status_code == 200:
            data = response.json()
            if data.get('update_available', False):
                new_version = data.get('latest_version', 'desconocida')
                logger.info(f"Nueva versión disponible: {new_version}")
                return data
            else:
                logger.info("El relay está actualizado")
                return None
        else:
            logger.warning(f"Error al verificar actualizaciones: HTTP {response.status_code}")
            return None
            
    except requests.exceptions.Timeout:
        logger.warning("Timeout al verificar actualizaciones")
        return None
    except requests.exceptions.ConnectionError:
        logger.warning("No se pudo conectar al servidor de actualizaciones")
        return None
    except Exception as e:
        logger.error(f"Error inesperado al verificar actualizaciones: {e}")
        return None


def verificar_hash(filepath: str, expected_hash: str) -> bool:
    """
    Verifica el hash SHA256 de un archivo descargado.
    
    Args:
        filepath: Ruta al archivo a verificar.
        expected_hash: Hash SHA256 esperado.
        
    Returns:
        True si el hash coincide, False en caso contrario.
    """
    try:
        sha256 = hashlib.sha256()
        with open(filepath, 'rb') as f:
            for chunk in iter(lambda: f.read(8192), b''):
                sha256.update(chunk)
        
        calculated_hash = sha256.hexdigest()
        match = calculated_hash.lower() == expected_hash.lower()
        
        if match:
            logger.info(f"Verificación de hash exitosa: {calculated_hash[:16]}...")
        else:
            logger.error(
                f"Hash no coincide: esperado={expected_hash[:16]}..., "
                f"calculado={calculated_hash[:16]}..."
            )
        
        return match
    except Exception as e:
        logger.error(f"Error al verificar hash de {filepath}: {e}")
        return False


# ==============================================================================
# DESCARGA Y EXTRACCIÓN
# ==============================================================================

def descargar_actualizacion(download_url: str, dest_path: str) -> bool:
    """
    Descarga el archivo de actualización desde el servidor.
    
    Args:
        download_url: URL de descarga.
        dest_path: Ruta de destino para guardar el archivo.
        
    Returns:
        True si la descarga fue exitosa, False en caso contrario.
    """
    try:
        logger.info(f"Descargando actualización desde {download_url}")
        
        response = requests.get(download_url, stream=True, timeout=120)
        
        if response.status_code == 200:
            total_size = int(response.headers.get('content-length', 0))
            downloaded = 0
            
            with open(dest_path, 'wb') as f:
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)
                        downloaded += len(chunk)
            
            file_size = os.path.getsize(dest_path)
            logger.info(f"Descarga completada: {file_size} bytes")
            return True
        else:
            logger.error(f"Error en descarga: HTTP {response.status_code}")
            return False
            
    except requests.exceptions.Timeout:
        logger.error("Timeout durante la descarga de actualización")
        return False
    except Exception as e:
        logger.error(f"Error al descargar actualización: {e}")
        return False


def extraer_zip(zip_path: str, extract_dir: str) -> bool:
    """
    Extrae un archivo ZIP de actualización.
    
    Args:
        zip_path: Ruta al archivo ZIP.
        extract_dir: Directorio de destino para la extracción.
        
    Returns:
        True si la extracción fue exitosa, False en caso contrario.
    """
    try:
        with zipfile.ZipFile(zip_path, 'r') as z:
            # Verificar que el ZIP no está corrupto
            bad_file = z.testzip()
            if bad_file:
                logger.error(f"Archivo corrupto en ZIP: {bad_file}")
                return False
            
            z.extractall(extract_dir)
            logger.info(f"ZIP extraído exitosamente en {extract_dir}")
            return True
    except zipfile.BadZipFile:
        logger.error(f"El archivo {zip_path} no es un ZIP válido")
        return False
    except Exception as e:
        logger.error(f"Error al extraer ZIP: {e}")
        return False


# ==============================================================================
# BACKUP Y LIMPIEZA
# ==============================================================================

def crear_backup(source_path: str, backup_dir: str, prefix: str = "ness_relay") -> Optional[str]:
    """
    Crea un backup del ejecutable actual antes de actualizar.
    
    Args:
        source_path: Ruta al archivo a respaldar.
        backup_dir: Directorio donde guardar el backup.
        prefix: Prefijo para el nombre del backup.
        
    Returns:
        Ruta del backup creado o None si falló.
    """
    try:
        if not os.path.exists(source_path):
            logger.warning(f"Archivo a respaldar no existe: {source_path}")
            return None
        
        os.makedirs(backup_dir, exist_ok=True)
        
        timestamp = time.strftime("%Y%m%d_%H%M%S")
        backup_name = f"{prefix}_backup_{timestamp}"
        
        # Si es archivo, copiar; si es directorio, copiar árbol
        if os.path.isfile(source_path):
            ext = os.path.splitext(source_path)[1]
            backup_path = os.path.join(backup_dir, f"{backup_name}{ext}")
            shutil.copy2(source_path, backup_path)
        else:
            backup_path = os.path.join(backup_dir, backup_name)
            shutil.copytree(source_path, backup_path)
        
        logger.info(f"Backup creado: {backup_path}")
        return backup_path
        
    except Exception as e:
        logger.error(f"Error al crear backup: {e}")
        return None


def limpiar_backups_antiguos(
    backup_dir: str,
    prefix: str = "ness_relay",
    max_backups: int = 5
) -> int:
    """
    Elimina backups antiguos manteniendo solo los más recientes.
    
    Args:
        backup_dir: Directorio de backups.
        prefix: Prefijo para filtrar backups.
        max_backups: Número máximo de backups a mantener.
        
    Returns:
        Número de backups eliminados.
    """
    try:
        if not os.path.exists(backup_dir):
            return 0
        
        # Listar archivos/directorios que empiezan con el prefijo de backup
        backup_pattern = f"{prefix}_backup_"
        backups = []
        
        for item in os.listdir(backup_dir):
            if item.startswith(backup_pattern):
                full_path = os.path.join(backup_dir, item)
                mtime = os.path.getmtime(full_path)
                backups.append((full_path, mtime))
        
        if len(backups) <= max_backups:
            return 0
        
        # Ordenar por fecha de modificación (más recientes primero)
        backups.sort(key=lambda x: x[1], reverse=True)
        
        # Eliminar los más antiguos
        deleted = 0
        for backup_path, _ in backups[max_backups:]:
            try:
                if os.path.isfile(backup_path):
                    os.remove(backup_path)
                else:
                    shutil.rmtree(backup_path)
                deleted += 1
                logger.info(f"Backup antiguo eliminado: {backup_path}")
            except Exception as e:
                logger.warning(f"No se pudo eliminar backup {backup_path}: {e}")
        
        return deleted
        
    except Exception as e:
        logger.error(f"Error al limpiar backups: {e}")
        return 0


# ==============================================================================
# GESTIÓN DE PROCESOS Y EJECUTABLES
# ==============================================================================

def verificar_procesos_relay_activos(nombre_proceso: str = "ness_relay") -> list:
    """
    Verifica si hay procesos del relay ejecutándose.
    
    Args:
        nombre_proceso: Nombre del proceso a buscar.
        
    Returns:
        Lista de PIDs de procesos encontrados (excluyendo el actual).
    """
    pids = []
    current_pid = os.getpid()
    
    try:
        result = subprocess.run(
            ['pgrep', '-f', nombre_proceso],
            capture_output=True,
            text=True,
            timeout=10
        )
        
        if result.returncode == 0:
            for line in result.stdout.strip().split('\n'):
                try:
                    pid = int(line.strip())
                    if pid != current_pid:
                        pids.append(pid)
                except ValueError:
                    continue
        
        if pids:
            logger.info(f"Procesos relay activos encontrados: {pids}")
        
    except FileNotFoundError:
        # pgrep no disponible (no Linux)
        logger.debug("pgrep no disponible en este sistema")
    except subprocess.TimeoutExpired:
        logger.warning("Timeout al verificar procesos activos")
    except Exception as e:
        logger.error(f"Error al verificar procesos: {e}")
    
    return pids


def procesar_actualizaciones_pendientes(directorio: str) -> bool:
    """
    Procesa actualizaciones descargadas que están pendientes de aplicar.
    
    Args:
        directorio: Directorio donde buscar actualizaciones pendientes.
        
    Returns:
        True si se procesó alguna actualización, False en caso contrario.
    """
    try:
        pending_dir = os.path.join(directorio, '.pending_update')
        
        if not os.path.exists(pending_dir):
            return False
        
        logger.info(f"Procesando actualización pendiente en {pending_dir}")
        
        # Buscar el ejecutable en el directorio pendiente
        for item in os.listdir(pending_dir):
            source = os.path.join(pending_dir, item)
            dest = os.path.join(directorio, item)
            
            try:
                if os.path.isfile(source):
                    shutil.copy2(source, dest)
                    # Dar permisos de ejecución en Linux
                    os.chmod(dest, 0o755)
                    logger.info(f"Actualización aplicada: {item}")
            except Exception as e:
                logger.error(f"Error al aplicar actualización para {item}: {e}")
                return False
        
        # Limpiar directorio pendiente
        shutil.rmtree(pending_dir, ignore_errors=True)
        logger.info("Directorio de actualización pendiente limpiado")
        return True
        
    except Exception as e:
        logger.error(f"Error al procesar actualizaciones pendientes: {e}")
        return False


def actualizar_ejecutables_relay(
    source_dir: str,
    dest_dir: str,
    nombre_ejecutable: str = "ness_relay"
) -> bool:
    """
    Reemplaza el ejecutable del relay con la nueva versión.
    
    Args:
        source_dir: Directorio con los nuevos archivos.
        dest_dir: Directorio de destino (donde está el ejecutable actual).
        nombre_ejecutable: Nombre base del ejecutable.
        
    Returns:
        True si el reemplazo fue exitoso, False en caso contrario.
    """
    try:
        # Buscar el ejecutable en el directorio fuente
        source_exec = None
        for item in os.listdir(source_dir):
            if item.startswith(nombre_ejecutable):
                source_exec = os.path.join(source_dir, item)
                break
        
        if not source_exec:
            logger.error(f"Ejecutable '{nombre_ejecutable}' no encontrado en {source_dir}")
            return False
        
        dest_exec = os.path.join(dest_dir, os.path.basename(source_exec))
        
        # Crear backup antes de reemplazar
        if os.path.exists(dest_exec):
            backup = crear_backup(dest_exec, os.path.join(dest_dir, 'backups'))
            if not backup:
                logger.warning("No se pudo crear backup, continuando de todas formas")
        
        # Copiar nuevo ejecutable
        shutil.copy2(source_exec, dest_exec)
        os.chmod(dest_exec, 0o755)
        
        logger.info(f"Ejecutable actualizado: {dest_exec}")
        return True
        
    except Exception as e:
        logger.error(f"Error al actualizar ejecutable: {e}")
        return False


# ==============================================================================
# FLUJO PRINCIPAL DE ACTUALIZACIÓN
# ==============================================================================

def actualizar_relay() -> bool:
    """
    Ejecuta el flujo completo de actualización del relay.
    
    1. Verifica si hay actualización disponible
    2. Descarga la nueva versión
    3. Verifica el hash del archivo descargado
    4. Crea backup del ejecutable actual
    5. Extrae y reemplaza los archivos
    6. Limpia archivos temporales
    
    Returns:
        True si la actualización fue exitosa, False en caso contrario.
    """
    logger.info("=" * 50)
    logger.info("INICIANDO PROCESO DE ACTUALIZACIÓN")
    logger.info("=" * 50)
    
    # 1. Verificar si hay actualización
    update_info = verificar_actualizacion()
    if not update_info:
        logger.info("No hay actualizaciones disponibles")
        return False
    
    new_version = update_info.get('latest_version', 'desconocida')
    download_url = update_info.get('download_url', '')
    expected_hash = update_info.get('sha256_hash', '')
    
    if not download_url:
        logger.error("URL de descarga no proporcionada por el servidor")
        return False
    
    logger.info(f"Actualizando: {RELAY_VERSION} -> {new_version}")
    
    # 2. Preparar directorio temporal
    temp_dir = str(BASE_DIR / '.update_temp')
    os.makedirs(temp_dir, exist_ok=True)
    
    zip_path = os.path.join(temp_dir, f"ness_relay_{new_version}.zip")
    extract_dir = os.path.join(temp_dir, 'extracted')
    
    try:
        # 3. Descargar
        if not descargar_actualizacion(download_url, zip_path):
            logger.error("Fallo en descarga de actualización")
            return False
        
        # 4. Verificar hash (si se proporcionó)
        if expected_hash:
            if not verificar_hash(zip_path, expected_hash):
                logger.error("Verificación de hash fallida - archivo corrupto o manipulado")
                return False
        else:
            logger.warning("No se proporcionó hash SHA256 - omitiendo verificación")
        
        # 5. Crear backup
        directorio_actual = str(BASE_DIR)
        limpiar_backups_antiguos(directorio_actual, "ness_relay", max_backups=5)
        
        if getattr(sys, 'frozen', False):
            crear_backup(sys.executable, os.path.join(directorio_actual, 'backups'))
        
        # 6. Extraer
        os.makedirs(extract_dir, exist_ok=True)
        if not extraer_zip(zip_path, extract_dir):
            logger.error("Fallo en extracción de actualización")
            return False
        
        # 7. Reemplazar ejecutable
        if not actualizar_ejecutables_relay(extract_dir, directorio_actual):
            logger.error("Fallo al reemplazar ejecutable")
            return False
        
        logger.info(f"Actualización exitosa: {RELAY_VERSION} -> {new_version}")
        logger.info("=" * 50)
        return True
        
    except Exception as e:
        logger.exception(f"Error durante actualización: {e}")
        return False
    finally:
        # Limpiar archivos temporales
        try:
            if os.path.exists(temp_dir):
                shutil.rmtree(temp_dir, ignore_errors=True)
        except Exception:
            pass


def obtener_version_real() -> str:
    """
    Obtiene la versión real del relay, verificando si hay una actualización
    aplicada que cambió la versión.
    
    Returns:
        String con la versión actual del relay.
    """
    try:
        # Verificar si existe un archivo de versión actualizado
        version_file = BASE_DIR / '.version'
        if version_file.exists():
            stored_version = version_file.read_text(encoding='utf-8').strip()
            if stored_version:
                return stored_version
    except Exception:
        pass
    
    return RELAY_VERSION
