#!/usr/bin/env python3
# ==============================================================================
# NESS Relay v2.0.0 - Multi-Vendor Enterprise SNMP Monitor
# ==============================================================================
#
# Punto de entrada principal del relay. Gestiona:
# - Inicialización del entorno (crypto, logging, profiles)
# - Parsing de argumentos de línea de comandos
# - Carga de configuración de dispositivos
# - Orquestación de la recolección via CollectionEngine
#
# Uso:
#   python ness_relay.py                          # Monitoreo con devices.conf
#   python ness_relay.py --config /ruta/devices.conf
#   python ness_relay.py --version                # Mostrar versión
#   python ness_relay.py --update                 # Verificar actualizaciones
#
# ==============================================================================

# === PASO 0: Inicialización pre-import ===
# El crypto backend DEBE cargarse ANTES de importar pysnmp para que
# SNMPv3 funcione correctamente en binarios PyInstaller.

# Este es un comentario de prueba
from utils.crypto_init import init_crypto_backend, setup_unbuffered_output, suppress_warnings

suppress_warnings()
setup_unbuffered_output()
init_crypto_backend()

# === Imports estándar ===
import argparse
import asyncio
import sys

# === Imports del proyecto ===
from core.config import (
    BASE_DIR,
    LOG_FILE,
    RELAY_TYPE,
    RELAY_VERSION,
    SERVER_ID,
    get_server_display_name,
    get_snmp_config_from_env,
    load_devices_from_config,
)
from core.engine import CollectionEngine
from core.logging_setup import setup_logging
from core.updater import (
    actualizar_relay,
    limpiar_backups_antiguos,
    obtener_version_real,
    verificar_actualizacion,
)
from profiles.profile_loader import load_all_profiles
from utils.helpers import print_simple


# ==============================================================================
# MAIN
# ==============================================================================

async def main() -> None:
    """Función principal del relay."""
    
    # 1. Configurar logging
    logger = setup_logging(log_file=LOG_FILE)
    
    # 2. Registrar perfiles de vendor
    load_all_profiles()
    
    # 3. Log de servidor configurado
    server_name = get_server_display_name()
    logger.info(f"Servidor configurado: {server_name} (ID: {SERVER_ID})")
    
    # 4. Parsear argumentos
    parser = argparse.ArgumentParser(
        description='NESS Relay v2.0.0 - Monitor SNMP Multi-Vendor Enterprise'
    )
    parser.add_argument(
        '--config', '-c',
        type=str,
        default='configs/devices.conf',
        help='Archivo de configuración de dispositivos (default: configs/devices.conf)'
    )
    parser.add_argument(
        '--version', '-v',
        action='store_true',
        help='Mostrar versión del relay y salir'
    )
    parser.add_argument(
        '--update', '-u',
        action='store_true',
        help='Verificar y realizar actualización si hay disponible'
    )
    parser.add_argument(
        '--silent', '-s',
        action='store_true',
        help='Modo silencioso (sin salida a consola)'
    )
    parser.add_argument(
        '--continuous',
        type=int,
        metavar='MINUTES',
        default=0,
        help='Monitoreo continuo cada N minutos (default: desactivado)'
    )
    
    args = parser.parse_args()
    
    # =========================================================================
    # COMANDOS QUE NO REQUIEREN CONFIGURACIÓN SNMP
    # =========================================================================
    
    # --- Mostrar versión ---
    if args.version:
        version_real = obtener_version_real()
        if not args.silent:
            print(f"NESS Relay v{version_real}")
            print(f"Tipo: {RELAY_TYPE}")
            print(f"Arquitectura: Multi-Vendor Enterprise")
        logger.info(f"Versión del relay: {version_real}")
        return
    
    # --- Actualización ---
    if args.update:
        logger.info("Verificando actualizaciones por solicitud del usuario")
        
        try:
            directorio_actual = str(BASE_DIR)
            limpiar_backups_antiguos(directorio_actual, "ness_relay", max_backups=5)
        except Exception as e:
            logger.warning(f"Error al limpiar backups: {e}")
        
        update_result = actualizar_relay()
        if update_result:
            if not args.silent:
                print("✅ Actualización completada exitosamente")
            logger.info("Actualización completada exitosamente")
        else:
            version_info = verificar_actualizacion()
            if version_info is None:
                if not args.silent:
                    print(f"✓ El relay ya está actualizado (versión {RELAY_VERSION})")
                logger.info(f"Relay en la versión más reciente: {RELAY_VERSION}")
            else:
                if not args.silent:
                    print("❌ Error durante la actualización")
                logger.error("Error durante el proceso de actualización")
        return
    
    # =========================================================================
    # COMANDOS QUE REQUIEREN CONFIGURACIÓN SNMP
    # =========================================================================
    
    # Crear motor de recolección
    engine = CollectionEngine()
    
    # Intentar cargar dispositivos desde archivo de configuración
    devices = load_devices_from_config(args.config)
    
    if not devices:
        # Fallback: intentar variables de entorno
        logger.warning("No se encontraron dispositivos en el archivo de configuración")
        logger.info("Intentando usar variables de entorno...")
        
        host, community, port = get_snmp_config_from_env()
        
        if host:
            # Crear dispositivo desde variables de entorno
            devices = [{
                'ip': host,
                'port': port,
                'community': community,
                'vendor': 'pfsense',  # Default para compatibilidad con v1.0.4
                'snmp_version': '2c',
                'description': f'Dispositivo {host} (env vars)',
            }]
            logger.info(f"Usando configuración de entorno - Host: {host}, Port: {port}")
        else:
            logger.error(
                "No se pudo obtener configuración SNMP. "
                "Configure devices.conf o variables de entorno."
            )
            print_simple("=" * 80)
            print_simple("❌ ERROR: No hay configuración de dispositivos")
            print_simple("=" * 80)
            print_simple("No se encontró archivo de configuración ni variables de entorno.")
            print_simple("")
            print_simple("Opciones:")
            print_simple(f"  1. Use: {sys.argv[0]} --config /ruta/a/devices.conf")
            print_simple("  2. Configure las variables de entorno: SNMP_HOST, SNMP_COMMUNITY, SNMP_PORT")
            print_simple("=" * 80)
            sys.exit(1)
    
    logger.info(f"Se encontraron {len(devices)} dispositivo(s) para monitorear")
    
    # =========================================================================
    # EJECUCIÓN
    # =========================================================================
    
    if args.continuous > 0:
        # Modo monitoreo continuo
        await engine.continuous_monitoring(devices, interval_minutes=args.continuous)
    else:
        # Ejecución única
        successful, failed = await engine.collect_all_devices(devices)
        
        # Resumen final
        total = successful + failed
        if total > 0:
            print_simple("")
            print_simple("=" * 80)
            print_simple("RESUMEN FINAL DE TODOS LOS DISPOSITIVOS")
            print_simple("=" * 80)
            print_simple(f"Total de dispositivos: {total}")
            print_simple(f"✓ Exitosos: {successful}")
            if failed > 0:
                print_simple(f"❌ Fallidos: {failed}")
            print_simple("=" * 80)
            
            if failed > 0:
                logger.warning(
                    f"Ejecución completada con errores: "
                    f"{failed}/{total} dispositivos fallaron"
                )
                sys.exit(1)
            else:
                logger.info(
                    f"Ejecución completada exitosamente: "
                    f"{successful}/{total} dispositivos procesados"
                )
                sys.exit(0)


# ==============================================================================
# ENTRY POINT
# ==============================================================================

if __name__ == "__main__":
    asyncio.run(main())
