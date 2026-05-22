localhost:/home/nessdeployer/Escritorio/relay_rust # ls
clean_relay.sh  install_relay.sh  ness-relay-x86_64
localhost:/home/nessdeployer/Escritorio/relay_rust # ./ness-relay-x86_64 --version
NESS Relay Multi-Vendor v2.0.0 (ness-relay)


Ejecutando relay por primera vez...

2026-05-17T21:15:36.875880Z  INFO NESS Relay v2.0.0 iniciando — servidor: http://172.206.0.217:8080/api/relay/data/
2026-05-17T21:15:36.876140Z  INFO Iniciando ciclo de recolección — 1 dispositivo(s)
2026-05-17T21:15:36.876264Z  INFO [mikrotik_fw_1] Iniciando recolección — vendor=mikrotik_fw ip=10.10.10.1
2026-05-17T21:15:36.876389Z  INFO [mikrotik_fw_1] [1/8] Perfil cargado: MikroTik Firewall (RouterOS)
2026-05-17T21:15:36.876418Z  INFO [mikrotik_fw_1] [2/8] Probando conectividad SNMP…
2026-05-17T21:15:36.882916Z  INFO [mikrotik_fw_1] [2/8] Conectividad OK
2026-05-17T21:15:36.889154Z  INFO [mikrotik_fw_1] [2/8] Perfil resuelto: MikroTik Firewall (RouterOS) (mikrotik_fw)
2026-05-17T21:15:36.889178Z  INFO [mikrotik_fw_1] [3/8] Recolectando sistema…
2026-05-17T21:15:36.907581Z  INFO [mikrotik_fw_1] [4/8] Recolectando performance…
2026-05-17T21:15:36.942746Z  INFO [mikrotik_fw_1] [5/8] Recolectando interfaces…
2026-05-17T21:15:37.135576Z  INFO [mikrotik_fw_1] [6/8] Recolectando seguridad…
2026-05-17T21:15:37.187733Z  INFO [mikrotik_fw_1] [7/8] Recolectando datos del vendor…
2026-05-17T21:15:37.357398Z  INFO [mikrotik_fw_1] [8/8] Analizando alertas…
2026-05-17T21:15:37.357601Z  INFO [mikrotik_fw_1] Recolección completada — 0 alertas, 0 advertencias en 0.5s
2026-05-17T21:15:37.726018Z  INFO Datos enviados correctamente al servidor NESS (HTTP 200)
2026-05-17T21:15:37.726426Z  INFO [mikrotik_fw_1] Datos enviados al servidor NESS correctamente.
2026-05-17T21:15:37.728534Z  INFO Ciclo completado — 1 exitoso(s), 0 fallido(s)
2026-05-17T21:15:37.728565Z  INFO Ciclo de recolección completado.


{
  "metadata": {
    "collection_duration_seconds": 0.48,
    "collection_end": "2026-05-17T16:15:37-05:00",
    "collection_start": "2026-05-17T16:15:36-05:00",
    "description": "",
    "device_type": "firewall",
    "relay_type": "ness-relay",
    "relay_version": "2.0.0",
    "snmp_host": "10.10.10.1",
    "snmp_port": 161,
    "total_interfaces": 3,
    "vendor": "mikrotik_fw",
    "vendor_display_name": "MikroTik Firewall (RouterOS)"
  },
  "mikrotik_fw_specific": {
    "collection_timestamp": "2026-05-17T21:15:37.187896604+00:00",
    "cpu_detailed": {
      "average_percent": 5.0,
      "core_count": 1,
      "cores": [
        {
          "index": "1",
          "load_percent": 5.0
        }
      ]
    },
    "disk_fallback": {
      "free_gb": 0.0,
      "mtxr_hl_disk_total": 0,
      "mtxr_hl_disk_used": 0,
      "percent_used": 0.0,
      "total_gb": 0.0,
      "used_gb": 0.0
    },
    "health": {},
    "interfaces_total": 3,
    "internet_channels": {
      "channels": [
        {
          "alerts": [],
          "channel_name": "ether1",
          "discards_in": 0,
          "discards_out": 0,
          "errors_in": 0,
          "errors_out": 0,
          "is_up": true,
          "isp": "Desconocido",
          "netwatch_status": null,
          "oper_status": "UP",
          "source": "wan_interface",
          "speed_mbps": 1000,
          "traffic_in_mb": 20.48,
          "traffic_out_mb": 0.0
        }
      ],
      "summary": {
        "channels_down": 0,
        "channels_up": 1,
        "netwatch_available": false,
        "queues_available": false,
        "total_channels": 1,
        "total_traffic_in_mb": 20.48,
        "total_traffic_out_mb": 0.0
      }
    },
    "netwatch": {
      "available": false,
      "probes": [],
      "summary": {
        "availability_percent": null,
        "down": 0,
        "total": 0,
        "up": 0
      }
    },
    "queues": {
      "available": false,
      "entries": [],
      "summary": {
        "total_queues": 0,
        "total_rx_drops": 0,
        "total_rx_gb": -0.0,
        "total_tx_drops": 0,
        "total_tx_gb": -0.0
      }
    },
    "system_info": {
      "mtxr_board_name": "CHR innotek GmbH VirtualBox",
      "mtxr_firmware_version": "7.22.2",
      "mtxr_license_id": "0"
    },
    "wan_interfaces": [
      {
        "admin_status": "UP",
        "alias": null,
        "discards_in": 0,
        "discards_out": 0,
        "errors_in": 0,
        "errors_out": 0,
        "if_name": "ether1",
        "index": "2",
        "is_wan": true,
        "isp_detected": null,
        "name": "ether1",
        "oper_status": "UP",
        "packets_in": 33033,
        "packets_out": 18672,
        "speed_mbps": 1000,
        "traffic_in_bytes": 17726442,
        "traffic_in_mb": 20.48,
        "traffic_out_bytes": 1734595,
        "traffic_out_mb": 0.0
      }
    ]
  },
  "network": {
    "collection_timestamp": "2026-05-17T16:15:37-05:00",
    "interfaces": {
      "1": {
        "admin_status": "UP",
        "discards_in": 0,
        "discards_out": 0,
        "errors_in": 0,
        "errors_out": 0,
        "index": "1",
        "name": "lo",
        "operational_status": "UP",
        "speed_mbps": 0,
        "total_errors": 0,
        "traffic_in_mb": 0.0,
        "traffic_out_mb": 0.0
      },
      "2": {
        "admin_status": "UP",
        "discards_in": 0,
        "discards_out": 0,
        "errors_in": 0,
        "errors_out": 0,
        "index": "2",
        "name": "ether1",
        "operational_status": "UP",
        "speed_mbps": 1000,
        "total_errors": 0,
        "traffic_in_mb": 16.91,
        "traffic_out_mb": 1.65
      },
      "3": {
        "admin_status": "UP",
        "discards_in": 0,
        "discards_out": 0,
        "errors_in": 0,
        "errors_out": 0,
        "index": "3",
        "name": "ether2",
        "operational_status": "UP",
        "speed_mbps": 1000,
        "total_errors": 0,
        "traffic_in_mb": 1.81,
        "traffic_out_mb": 16.95
      }
    }
  },
  "performance": {
    "collection_timestamp": "2026-05-17T16:15:36-05:00",
    "cpu": {
      "cpu_core_count": 0,
      "cpu_cores": [],
      "cpu_usage_percent": 5.0,
      "load_15min": 0.0,
      "load_1min": 0.0,
      "load_5min": 0.0
    },
    "disk": {
      "1": {
        "free_gb": 0.07,
        "index": "1",
        "path": "system disk",
        "percent_used": 22.46,
        "source_raw": {
          "disk_percent_raw": 22.46,
          "disk_total_raw_gb": 0.09,
          "disk_used_raw_gb": 0.02
        },
        "total_gb": 0.09,
        "used_gb": 0.02
      }
    },
    "memory": {
      "mem_available_mb": 798.72,
      "mem_free_mb": 798.72,
      "mem_total_mb": 1024.0,
      "mem_usage_percent": 21.54,
      "mem_used_mb": 225.28,
      "swap_free_mb": 0.0,
      "swap_total_mb": 0.0
    }
  },
  "performance_analysis": {
    "alerts": [],
    "performance_status": "ok",
    "timestamp": "2026-05-17T16:15:37-05:00",
    "total_alerts": 0,
    "total_warnings": 0,
    "warnings": []
  },
  "security": {
    "collection_timestamp": "2026-05-17T16:15:37-05:00",
    "icmp_security": {
      "icmp_in_dest_unreachs": 0,
      "icmp_in_echo_reps": 0,
      "icmp_in_echos": 0,
      "icmp_in_errors": 0,
      "icmp_in_msgs": 0,
      "icmp_in_redirects": 0,
      "icmp_in_time_excds": 0
    },
    "ip_security": {
      "ip_frag_fails": 0,
      "ip_frag_oks": 0,
      "ip_in_addr_errors": 0,
      "ip_in_discards": 0,
      "ip_in_hdr_errors": 0,
      "ip_in_receives": 0,
      "ip_in_unknown_protos": 0
    },
    "normalized": {
      "icmp": {
        "echo_reply_rate_percent": 0.0,
        "in_echo_reps": 0,
        "in_echos": 0,
        "in_errors": 0,
        "in_msgs": 0
      },
      "ip": {
        "error_rate_percent": 0.0,
        "frag_fails": 0,
        "frag_oks": 0,
        "fragmentation_rate_percent": 0.0,
        "in_addr_errors": 0,
        "in_discards": 0,
        "in_hdr_errors": 0,
        "in_receives": 0,
        "in_unknown_protos": 0
      },
      "snmp": {
        "asn_parse_errs": 0,
        "bad_community_names": 0,
        "bad_community_rate_percent": 0.0,
        "bad_versions": 0,
        "gen_errs": 0,
        "in_pkts": 0
      },
      "tcp": {
        "active_opens": 0,
        "attempt_fails": 0,
        "current_estab": 0,
        "estab_resets": 0,
        "in_segs": 0,
        "out_rsts": 0,
        "out_segs": 0,
        "passive_opens": 0,
        "retrans_segs": 0,
        "retransmission_rate_percent": 0.0
      },
      "udp": {
        "error_rate_percent": 0.0,
        "in_datagrams": 0,
        "in_errors": 0,
        "no_ports": 0,
        "out_datagrams": 0
      }
    },
    "snmp_security": {
      "snmp_in_asn_parse_errs": 0,
      "snmp_in_bad_community_names": 0,
      "snmp_in_bad_community_uses": 0,
      "snmp_in_bad_versions": 0,
      "snmp_in_gen_errs": 0,
      "snmp_in_pkts": 0
    },
    "tcp_security": {
      "retransmission_rate_percent": 0.0,
      "tcp_active_opens": 0,
      "tcp_attempt_fails": 0,
      "tcp_curr_estab": 0,
      "tcp_estab_resets": 0,
      "tcp_in_segs": 0,
      "tcp_out_rsts": 0,
      "tcp_out_segs": 0,
      "tcp_passive_opens": 0,
      "tcp_retrans_segs": 0
    },
    "udp_security": {
      "udp_in_datagrams": 0,
      "udp_in_errors": 0,
      "udp_no_ports": 0,
      "udp_out_datagrams": 0
    }
  },
  "security_analysis": {
    "alerts": [],
    "security_status": "ok",
    "timestamp": "2026-05-17T16:15:37-05:00",
    "total_alerts": 0,
    "total_warnings": 0,
    "warnings": []
  },
  "system": {
    "basic_info": {
      "sys_contact": "relay-user",
      "sys_descr": "RouterOS CHR 7.22.2 (stable)",
      "sys_location": "Bogotá D.C.",
      "sys_name": "MikroTik",
      "sys_uptime": {
        "days": 0,
        "formatted": "0d 1h 38m 56s",
        "hours": 1,
        "human": "0d 1h 38m 56s",
        "minutes": 38,
        "seconds": 56,
        "total_seconds": 5936
      }
    },
    "collection_time_utc": "2026-05-17T21:15:36Z",
    "sys_object_id": "1.3.6.1.4.1.14988.1",
    "timestamp": "2026-05-17T16:15:36-05:00",
    "uptime_raw": 593600
  }
}                                                       