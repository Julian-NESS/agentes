# ==============================================================================
# NESS Relay v2.0.0 - Collection Engine
# ==============================================================================
# Motor de recolección vendor-agnostic. Orquesta el flujo completo:
# 1. Crea SnmpClient para el dispositivo
# 2. Carga el Profile del vendor
# 3. Ejecuta todos los collectors
# 4. Ejecuta los analyzers
# 5. Exporta y envía datos
#
# El engine NO sabe qué vendor está monitoreando. Le pregunta al Profile
# qué OIDs usar y cómo normalizar los datos.
# ==============================================================================

import asyncio
import logging
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

from analyzers.performance_analyzer import analyze_performance_metrics
from analyzers.security_analyzer import analyze_security_threats
from collectors.network_collector import collect_network_data
from collectors.performance_collector import collect_performance_data
from collectors.security_collector import collect_security_data
from collectors.system_collector import collect_system_data
from collectors.vendor_collector import collect_vendor_specific_data
from core.config import (
    JSON_FILE,
    LOG_FILE,
    OUTPUT_DIR,
    RELAY_TYPE,
    RELAY_VERSION,
)
from core.snmp_client import SnmpClient
from exporters.json_exporter import export_to_json
from exporters.server_sender import send_data_to_server
from profiles.base_profile import BaseDeviceProfile
from profiles.profile_loader import ProfileLoader
from utils.helpers import now_iso, print_simple

logger = logging.getLogger("ness_relay")


class CollectionEngine:
    """
    Motor de recolección vendor-agnostic.
    
    Orquesta la recolección de datos para uno o múltiples dispositivos.
    Es completamente independiente del vendor: delega al Profile la
    obtención de OIDs y la normalización de datos.
    """
    
    def __init__(self):
        """Inicializa el engine."""
        self.results: List[Dict[str, Any]] = []
    
    async def collect_device(self, device_config: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """
        Ejecuta el flujo completo de recolección para UN dispositivo.
        
        Pasos:
        1. Cargar perfil del vendor
        2. Crear cliente SNMP
        3. Verificar conectividad
        4. Recolectar datos (system, performance, network, security, vendor)
        5. Analizar amenazas y performance
        6. Exportar a JSON
        7. Enviar al servidor
        
        Args:
            device_config: Configuración del dispositivo (de devices.conf).
            
        Returns:
            Diccionario con todos los datos recolectados, o None si falló.
        """
        start_time = time.time()
        vendor = device_config.get('vendor', 'generic')
        description = device_config.get('description', device_config.get('ip', 'unknown'))
        
        print_simple("=" * 80)
        print_simple(f"Iniciando recolección de datos - {description}")
        print_simple("=" * 80)
        
        # 1. Cargar perfil del vendor
        try:
            profile = ProfileLoader.get_profile(vendor)
        except ValueError as e:
            logger.error(f"No se encontró perfil para vendor '{vendor}': {e}")
            print_simple(f"❌ ERROR: Vendor '{vendor}' no soportado")
            return None
        
        logger.info(f"Perfil cargado: {profile.vendor_display_name}")
        
        # 2. Crear cliente SNMP (async factory - pysnmp v7.x)
        client = await SnmpClient.create(device_config)
        conn_info = client.get_connection_info()
        
        logger.info(f"Conectando a: {conn_info['host']}:{conn_info['port']}")
        print_simple(f"Host: {conn_info['host']}:{conn_info['port']}")
        print_simple(f"Versión SNMP: {conn_info['snmp_version']}")
        print_simple(f"Vendor: {profile.vendor_display_name}")
        
        if client.snmp_version == '3':
            print_simple(f"Usuario SNMPv3: {conn_info.get('v3_user', 'N/A')}")
            print_simple(f"Autenticación: {conn_info.get('v3_auth', 'N/A')}")
            print_simple(f"Privacidad: {conn_info.get('v3_priv', 'N/A')}")
        else:
            print_simple(f"Community: {conn_info.get('community', 'N/A')}")
        
        print_simple("")
        
        # 3. Verificar conectividad SNMP
        print_simple("[1/8] Verificando conectividad SNMP...")
        test_result = await client.test_connectivity()
        
        if test_result.error:
            logger.error(f"Fallo en conectividad SNMP: {test_result.error}")
            print_simple("❌ ERROR: No se pudo conectar al dispositivo")
            print_simple(f"Detalle: {test_result.error}")
            print_simple("")
            self._print_connectivity_help(client)
            return None
        
        print_simple(f"✓ Conectividad SNMP OK: {test_result.value}")
        logger.info(f"Conectividad SNMP establecida con {test_result.value}")
        
        # 4. Preparar estructura de datos
        all_data: Dict[str, Any] = {
            "metadata": {
                "collection_start": now_iso(),
                "snmp_host": client.host,
                "snmp_port": client.port,
                "vendor": profile.vendor,
                "vendor_display_name": profile.vendor_display_name,
                "device_type": profile.device_type,
                "relay_version": RELAY_VERSION,
                "relay_type": RELAY_TYPE,
                "description": description,
            }
        }
        
        try:
            # 5. Ejecutar collectors
            print_simple("[2/8] Recolectando datos del sistema...")
            all_data["system"] = await collect_system_data(client)
            print_simple("✓ Datos del sistema OK")
            
            print_simple("[3/8] Recolectando métricas de rendimiento...")
            all_data["performance"] = await collect_performance_data(client, profile)
            print_simple("✓ Datos de rendimiento OK")
            
            print_simple("[4/8] Recolectando información de red...")
            all_data["network"] = await collect_network_data(client)
            interfaces_count = len(all_data["network"].get("interfaces", {}))
            print_simple(f"✓ Datos de red OK ({interfaces_count} interfaces)")
            
            print_simple("[5/8] Recolectando datos de seguridad...")
            all_data["security"] = await collect_security_data(client)
            print_simple("✓ Datos de seguridad OK")
            
            print_simple("[6/8] Recolectando datos específicos del dispositivo...")
            vendor_data = await collect_vendor_specific_data(client, profile)
            # Usar nombre del vendor como clave (ej: "pfsense_specific")
            all_data[f"{profile.vendor}_specific"] = vendor_data
            print_simple("✓ Datos específicos OK")
            
            # Post-procesamiento final: permite al perfil usar vendor data
            # para enriquecer/corregir datos de performance (ej: CPU de MikroTik)
            all_data = profile.finalize_collected_data(all_data)
            
            # 6. Ejecutar analyzers
            print_simple("[7/8] Analizando amenazas de seguridad...")
            security_analysis = analyze_security_threats(all_data)
            all_data["security_analysis"] = security_analysis
            sec_alerts = security_analysis.get("total_alerts", 0)
            print_simple(f"✓ Análisis de seguridad completo ({sec_alerts} alertas)")
            
            print_simple("[8/8] Analizando métricas de rendimiento...")
            performance_analysis = analyze_performance_metrics(all_data)
            all_data["performance_analysis"] = performance_analysis
            perf_alerts = performance_analysis.get("total_alerts", 0)
            print_simple(f"✓ Análisis de rendimiento completo ({perf_alerts} alertas)")
            
            # 7. Metadata de cierre
            end_time = time.time()
            all_data["metadata"]["collection_end"] = now_iso()
            all_data["metadata"]["collection_duration_seconds"] = round(end_time - start_time, 2)
            all_data["metadata"]["total_interfaces"] = interfaces_count
            
            # 8. Exportar a JSON
            print_simple("")
            print_simple("Guardando datos localmente...")
            json_file = export_to_json(all_data, JSON_FILE)
            if not json_file:
                print_simple("❌ ERROR: No se pudo guardar los datos localmente")
                logger.error("Error al guardar datos en archivo JSON")
                return None
            print_simple(f"✓ Datos guardados en: {json_file}")
            
            # 9. Enviar al servidor
            print_simple("")
            print_simple("Enviando datos al servidor NESS...")
            send_result = send_data_to_server(all_data)
            if not send_result:
                print_simple("❌ ERROR: No se pudieron enviar los datos al servidor")
                print_simple("Los datos están guardados localmente y se reintentará en la próxima ejecución")
                logger.error("Error al enviar datos al servidor")
            else:
                print_simple("✓ Datos enviados exitosamente al servidor")
            
            # 10. Resumen
            self._print_execution_summary(all_data, security_analysis, performance_analysis)
            
            logger.info(
                f"Recolección completada exitosamente en "
                f"{all_data['metadata']['collection_duration_seconds']}s"
            )
            return all_data
            
        except Exception as e:
            logger.exception(f"Error durante la recolección de datos: {e}")
            print_simple("")
            print_simple("=" * 80)
            print_simple("❌ ERROR DURANTE LA EJECUCIÓN")
            print_simple("=" * 80)
            print_simple(f"Error: {str(e)}")
            print_simple(f"Log detallado: {LOG_FILE}")
            print_simple("=" * 80)
            return None
    
    async def collect_all_devices(
        self,
        devices: List[Dict[str, Any]]
    ) -> tuple:
        """
        Ejecuta la recolección para todos los dispositivos configurados.
        
        Args:
            devices: Lista de configuraciones de dispositivos.
            
        Returns:
            Tupla (successful_count, failed_count).
        """
        successful = 0
        failed = 0
        
        for device in devices:
            result = await self.collect_device(device)
            if result:
                self.results.append(result)
                successful += 1
            else:
                failed += 1
        
        return successful, failed
    
    async def continuous_monitoring(
        self,
        devices: List[Dict[str, Any]],
        interval_minutes: int = 5,
    ) -> None:
        """
        Ejecuta monitoreo continuo para todos los dispositivos.
        
        Args:
            devices: Lista de configuraciones de dispositivos.
            interval_minutes: Intervalo entre iteraciones en minutos.
        """
        print_simple(f"Iniciando monitoreo continuo (cada {interval_minutes} minutos)")
        print_simple("Presiona Ctrl+C para detener")
        
        iteration = 0
        try:
            while True:
                iteration += 1
                timestamp = datetime.now().strftime("%H:%M:%S")
                print_simple(f"[{timestamp}] Ejecutando monitoreo #{iteration}...")
                
                successful, failed = await self.collect_all_devices(devices)
                
                if successful > 0:
                    print_simple(f"✓ {successful} dispositivo(s) monitoreado(s)")
                if failed > 0:
                    print_simple(f"❌ {failed} dispositivo(s) fallaron")
                
                print_simple(f"Esperando {interval_minutes} minutos...")
                await asyncio.sleep(interval_minutes * 60)
                
        except KeyboardInterrupt:
            print_simple(
                f"Monitoreo detenido (después de {iteration} iteraciones)"
            )
            logger.info(
                f"Monitoreo continuo detenido después de {iteration} iteraciones"
            )
    
    # ==========================================================================
    # HELPERS PRIVADOS
    # ==========================================================================
    
    @staticmethod
    def _print_connectivity_help(client: SnmpClient) -> None:
        """Muestra ayuda cuando falla la conectividad SNMP."""
        print_simple("Posibles causas:")
        if client.snmp_version == '3':
            print_simple("  - Usuario SNMPv3 incorrecto")
            print_simple("  - Contraseña de autenticación incorrecta")
            print_simple("  - Contraseña de privacidad incorrecta")
            print_simple("  - Protocolo de autenticación no coincide (MD5/SHA)")
            print_simple("  - Protocolo de privacidad no coincide (DES/AES)")
        else:
            print_simple("  - Community string incorrecto")
            print_simple("  - SNMP no habilitado en el dispositivo")
        print_simple("  - Firewall bloqueando puerto 161/UDP")
        print_simple("  - Host no alcanzable")
        print_simple("")
        print_simple(f"Revise el log detallado en: {LOG_FILE}")
    
    @staticmethod
    def _print_execution_summary(
        data: Dict[str, Any],
        security_analysis: Dict[str, Any],
        performance_analysis: Dict[str, Any],
    ) -> None:
        """Muestra resumen de ejecución."""
        print_simple("")
        print_simple("=" * 80)
        print_simple("RESUMEN DE EJECUCIÓN")
        print_simple("=" * 80)
        
        sec_alerts = security_analysis.get("total_alerts", 0)
        perf_alerts = performance_analysis.get("total_alerts", 0)
        total_alerts = sec_alerts + perf_alerts
        
        if total_alerts > 0:
            print_simple(
                f"⚠️  ALERTAS DETECTADAS: {total_alerts} "
                f"({sec_alerts} seguridad, {perf_alerts} rendimiento)"
            )
        else:
            print_simple("✓ Sin alertas detectadas")
        
        duration = data.get('metadata', {}).get('collection_duration_seconds', 'N/A')
        interfaces = data.get('metadata', {}).get('total_interfaces', 0)
        
        print_simple(f"✓ Tiempo de ejecución: {duration}s")
        print_simple(f"✓ Interfaces monitoreadas: {interfaces}")
        print_simple(f"✓ Log detallado: {LOG_FILE}")
        print_simple("=" * 80)
        print_simple("✅ EJECUCIÓN COMPLETADA EXITOSAMENTE")
        print_simple("=" * 80)
