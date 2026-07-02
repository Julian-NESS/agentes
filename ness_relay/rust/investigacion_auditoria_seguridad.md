# **Investigación: Expansión del Agente NESS Relay para Auditoría de Vulnerabilidades**

Este documento analiza la viabilidad, los objetivos de escaneo y la arquitectura de software necesaria para implementar módulos de ciberseguridad avanzada en el agente NESS Relay, enfocándose inicialmente en Firewalls (Fortinet, pfSense, MikroTik, Cisco) con capacidad de expansión a otros dispositivos de red.

## **1\. Viabilidad y Objetivos de Escaneo (Los 4 Aspectos)**

A continuación, resolvemos la duda principal: **¿Es posible escanear esto en firewalls?** La respuesta corta es **SÍ**, pero depende del tipo de firewall (cerrado/comercial vs. abierto/basado en Linux-FreeBSD).

### **Aspecto 1: Software de Terceros**

* **¿Es posible?:** Sí, pero depende del fabricante.  
  * **Firewalls Cerrados (Fortinet, Cisco ASA):** *No aplica directamente.* Estos equipos tienen sistemas operativos monolíticos (FortiOS) donde el usuario no instala "software de terceros" como tal.  
  * **Firewalls Abiertos (pfSense, OPNsense):** *Totalmente aplicable.* pfSense está basado en FreeBSD y permite instalar paquetes (ej. pfBlockerNG, Squid, Suricata).  
* **¿Qué escanear?:**  
  * Listado de paquetes instalados y sus versiones (Ej. en pfSense, ejecutar pkg info vía SSH o consultar la API de paquetes).  
  * Plugins o Add-ons instalados y su estado (activo/inactivo).

### **Aspecto 2: Actualizaciones del Sistema y KBs**

* **¿Es posible?:** **SÍ, es fundamental.**  
* **¿Qué escanear?:**  
  * **Versión del Firmware actual:** (Ej. FortiOS 7.0.5, RouterOS 7.8). Esto ya lo obtienes probablemente con SNMP (sysDescr), pero aquí le damos un uso de seguridad.  
  * **Parches Pendientes (KBs/Hotfixes):** En pfSense se puede consultar el estado de actualización (/api/v1/system/upgrade).  
  * **Fin de Vida (EOL/EOS):** El agente debe reportar la versión al servidor NESS, y el servidor (o el agente si descarga una base de datos local) debe contrastarla contra la base de datos de vulnerabilidades (CVEs asociadas a esa versión).

### **Aspecto 3: Controles del CIS (Center for Internet Security)**

* **¿Es posible?:** **SÍ, es el núcleo del Hardening.**  
* **¿Qué escanear?:** (Ejemplos de directivas de Nivel 1\)  
  * **Políticas de contraseñas:** Longitud mínima, complejidad exigida.  
  * **Tiempos de inactividad (Timeouts):** ¿Se cierra la sesión de admin tras 10 minutos de inactividad?  
  * **Protocolos de gestión seguros:** Validar que SSH esté en versión 2 (no v1), y que el cifrado usado no sea débil (ej. bloquear 3DES).  
  * **Auditoría y Logs:** Validar que el reenvío de logs (Syslog) hacia un servidor central (SIEM) esté configurado y habilitado.  
  * **Banners de advertencia:** Validar que exista un banner legal configurado al iniciar sesión (requisito clásico del CIS).

### **Aspecto 4: Servicios Mal Configurados**

* **¿Es posible?:** **SÍ, es la principal causa de brechas en firewalls.**  
* **¿Qué escanear?:**  
  * **Interfaces de administración expuestas:** Validar que servicios como SSH, HTTP/HTTPS o Ping *no estén habilitados en la interfaz WAN (pública)*.  
  * **Uso de protocolos en texto claro:** Validar que Telnet, HTTP y SNMP v1/v2c estén **deshabilitados**. Solo deben permitirse SSH, HTTPS y SNMPv3.  
  * **Reglas de Firewall permisivas:** Escanear si existe alguna regla tipo "Any-Any" (Permitir todo desde cualquier origen a cualquier destino) en interfaces que no sean de red de invitados.  
  * **Cuentas por defecto:** Validar que la cuenta admin por defecto haya sido renombrada o deshabilitada.

## **2\. Estrategia de Integración en el Agente (Arquitectura Rust)**

Para que el agente NESS Relay (v2.1.0) soporte esto sin romper el código actual y manteniendo buenas prácticas de diseño de software (SOLID), debemos abstraer el concepto de **Auditoría** separándolo de la **Recolección de Métricas**.

### **2.1. Evolución del Smart Tester**

Actualmente, el smart\_tester.rs prueba credenciales SNMP. Deberá expandirse para probar múltiples **Protocolos de Conexión (Transports)**.

* **Paso 1:** El usuario configura el equipo en la plataforma con credenciales SNMP, SSH y/o API Key.  
* **Paso 2:** El Smart Tester valida qué puertas están abiertas: "SNMP OK", "SSH OK", "API Falló".

### **2.2. Diseño Basado en Traits (Polimorfismo en Rust)**

Debemos crear una estructura modular. En lugar de ensuciar src/collectors/, crearemos un nuevo directorio: src/auditors/.

Implementaremos un Trait (Interfaz) que todos los fabricantes deben cumplir:

// src/auditors/mod.rs

use async\_trait::async\_trait;  
use serde\_json::Value;

/// Interfaz unificada para ejecutar auditorías de seguridad en cualquier dispositivo.  
\#\[async\_trait\]  
pub trait SecurityAuditor {  
    /// Escanea software de terceros (si aplica)  
    async fn scan\_third\_party\_software(\&self) \-\> Result\<Value, String\>;  
      
    /// Escanea estado de actualizaciones y versión  
    async fn scan\_system\_updates(\&self) \-\> Result\<Value, String\>;  
      
    /// Evalúa controles CIS específicos del dispositivo  
    async fn evaluate\_cis\_controls(\&self) \-\> Result\<Value, String\>;  
      
    /// Detecta servicios mal configurados (Telnet, interfaces expuestas)  
    async fn detect\_misconfigurations(\&self) \-\> Result\<Value, String\>;  
      
    /// Ejecuta todos los escaneos y consolida el reporte  
    async fn run\_full\_audit(\&self) \-\> Result\<Value, String\> {  
        // ... lógica para ejecutar los 4 métodos anteriores y unir el JSON  
        Ok(serde\_json::json\!({}))  
    }  
}

### **2.3. Implementación por Fabricante y Protocolo**

El agente instanciará el auditor adecuado según el perfil del dispositivo detectado.

* **Para pfSense (Usando SSH nativo):**  
  Crearás src/auditors/vendors/pfsense.rs. Este archivo implementará el trait SecurityAuditor. Usará una librería como russh o ssh2 para conectarse, enviar el comando pkg info \--json y analizar la respuesta para el Aspecto 1\.  
* **Para Fortinet (Usando API REST):**  
  Crearás src/auditors/vendors/fortinet.rs. Usará la librería reqwest nativa de Rust para hacer un GET https://\<ip\>/api/v2/cmdb/system/global y verificar en el JSON si admin-telnet está en falso (Aspecto 4).

### **2.4. Estructura de Directorios Propuesta**

Tu repositorio actual debería expandirse así:

ness\_relay\_v2.1.0/  
├── src/  
│   ├── collectors/      \# (Mantenido) Métricas SNMP de rendimiento y tráfico  
│   ├── analyzers/       \# (Mantenido) Umbrales de CPU, RAM, etc.  
│   ├── core/  
│   │   ├── smart\_tester.rs \# \-\> Modificado para probar SSH y HTTPs además de SNMP  
│   ├── auditors/        \# NUEVO MÓDULO DE CIBERSEGURIDAD  
│   │   ├── mod.rs       \# Trait SecurityAuditor y estructuras base  
│   │   ├── ssh\_client.rs\# NUEVO: Cliente SSH genérico reutilizable  
│   │   ├── api\_client.rs\# NUEVO: Cliente REST genérico reutilizable  
│   │   ├── vendors/  
│   │   │   ├── mod.rs  
│   │   │   ├── fortinet.rs \# Implementa SecurityAuditor vía API  
│   │   │   ├── pfsense.rs  \# Implementa SecurityAuditor vía SSH/XMLRPC  
│   │   │   ├── mikrotik.rs \# Implementa SecurityAuditor vía API o SSH

## **3\. Conclusión de la Investigación**

1. **Integración:** Es totalmente factible. La clave del éxito radicará en no intentar hacerlo todo por SNMP, sino dotar al agente de capacidades **SSH y REST API**.  
2. **Modularidad:** Al usar Traits de Rust, si mañana deseas auditar un Switch Cisco, solo creas src/auditors/vendors/cisco\_switch.rs, implementas las 4 funciones obligatorias, y la arquitectura general del agente NESS no sufrirá alteraciones.  
3. **Procesamiento:** El agente se vuelve un recolector inteligente. Ejecuta comandos/APIs, estructura los datos en formato JSON estandarizado, y los envía al servidor (Platform) donde se realiza el cruce contra las bases de datos de CVEs (Vulnerabilidades) globales.