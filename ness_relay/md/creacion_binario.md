warning: unused variable: `table_oid`
  --> src/collectors/performance.rs:85:17
   |
85 |     if let Some(table_oid) = disk_oids.values().find(|oid| {
   |                 ^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_table_oid`
   |
   = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `ip_frag_ok`
  --> src/collectors/security.rs:70:9
   |
70 |     let ip_frag_ok   = get_i64!(ip_o, "ipFragOKs");
   |         ^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_ip_frag_ok`

warning: unused variable: `client`
  --> src/profiles/vendors/cisco.rs:71:50
   |
71 |     async fn collect_vendor_specific_data(&self, client: &SnmpClient) -> serde_json::Value {
   |                                                  ^^^^^^ help: if this is intentional, prefix it with an underscore: `_client`

warning: unused variable: `poe_port_class`
   --> src/profiles/vendors/ubnt.rs:203:14
    |
203 |         let (poe_port_class, _)  = client.bulk("1.3.6.1.2.1.105.1.1.1.3", 30).await;
    |              ^^^^^^^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_poe_port_class`

warning: variable does not need to be mutable
   --> src/snmp/v3.rs:353:9
    |
353 |     let mut cipher = DesCbc::new(&key_arr.into(), &salt.into());
    |         ----^^^^^^
    |         |
    |         help: remove this `mut`
    |
    = note: `#[warn(unused_mut)]` (part of `#[warn(unused)]`) on by default

warning: unused variable: `consumed`
   --> src/snmp/mod.rs:335:57
    |
335 |         let time = if let Some((TAG_INTEGER, time_data, consumed)) =
    |                                                         ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_consumed`

warning: value assigned to `offset` is never read
   --> src/snmp/mod.rs:300:13
    |
300 |             offset += consumed;
    |             ^^^^^^^^^^^^^^^^^^
    |
    = help: maybe it is overwritten before being read?
    = note: `#[warn(unused_assignments)]` (part of `#[warn(unused)]`) on by default

warning: constant `MAX_BACKUPS` is never used
  --> src/config.rs:27:11
   |
27 | pub const MAX_BACKUPS: usize = 5;
   |           ^^^^^^^^^^^
   |
   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

warning: fields `server_id`, `version`, `relay_type`, `base_dir`, `hosting_base_url`, and `update_report_url` are never read
  --> src/config.rs:63:9
   |
59 | pub struct AppConfig {
   |            --------- fields in this struct
...
63 |     pub server_id: String,
   |         ^^^^^^^^^
...
67 |     pub version: String,
   |         ^^^^^^^
68 |     /// Tipo del relay
69 |     pub relay_type: String,
   |         ^^^^^^^^^^
70 |     /// Directorio base (donde está el ejecutable)
71 |     pub base_dir: PathBuf,
   |         ^^^^^^^^
...
81 |     pub hosting_base_url: String,
   |         ^^^^^^^^^^^^^^^^
82 |     /// URL para reportar actualizaciones realizadas
83 |     pub update_report_url: String,
   |         ^^^^^^^^^^^^^^^^^
   |
   = note: `AppConfig` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: method `version_check_url` is never used
   --> src/config.rs:139:12
    |
 86 | impl AppConfig {
    | -------------- method in this implementation
...
139 |     pub fn version_check_url(&self) -> &str {
    |            ^^^^^^^^^^^^^^^^^

warning: function `read_exported` is never used
  --> src/exporters/json_exporter.rs:35:14
   |
35 | pub async fn read_exported(output_dir: &str) -> Option<serde_json::Value> {
   |              ^^^^^^^^^^^^^

warning: function `ping` is never used
  --> src/exporters/server_sender.rs:64:14
   |
64 | pub async fn ping(server_url: &str, api_token: &str) -> bool {
   |              ^^^^

warning: methods `get_vendor_oids` and `matches_sys_object_id` are never used
   --> src/profiles/base.rs:51:8
    |
 23 | pub trait DeviceProfile: Send + Sync {
    |           ------------- methods in this trait
...
 51 |     fn get_vendor_oids(&self) -> HashMap<String, String> {
    |        ^^^^^^^^^^^^^^^
...
100 |     fn matches_sys_object_id(&self, _sys_object_id: &str) -> bool {
    |        ^^^^^^^^^^^^^^^^^^^^^

warning: methods `list_vendors` and `auto_detect` are never used
  --> src/profiles/loader.rs:76:12
   |
31 | impl ProfileLoader {
   | ------------------ methods in this implementation
...
76 |     pub fn list_vendors(&self) -> Vec<&String> {
   |            ^^^^^^^^^^^^
...
84 |     pub fn auto_detect(&self, sys_object_id: &str) -> Option<Arc<dyn DeviceProfile>> {
   |            ^^^^^^^^^^^

warning: function `hr_storage_oids` is never used
   --> src/profiles/standard_oids.rs:160:8
    |
160 | pub fn hr_storage_oids() -> HashMap<&'static str, &'static str> {
    |        ^^^^^^^^^^^^^^^

warning: function `hr_processor_oids` is never used
   --> src/profiles/standard_oids.rs:172:8
    |
172 | pub fn hr_processor_oids() -> HashMap<&'static str, &'static str> {
    |        ^^^^^^^^^^^^^^^^^

warning: constant `RAM` is never used
   --> src/profiles/standard_oids.rs:183:15
    |
183 |     pub const RAM: &str              = "1.3.6.1.2.1.25.2.1.2";
    |               ^^^

warning: constant `VIRTUAL_MEMORY` is never used
   --> src/profiles/standard_oids.rs:184:15
    |
184 |     pub const VIRTUAL_MEMORY: &str  = "1.3.6.1.2.1.25.2.1.3";
    |               ^^^^^^^^^^^^^^

warning: constant `FIXED_DISK` is never used
   --> src/profiles/standard_oids.rs:185:15
    |
185 |     pub const FIXED_DISK: &str      = "1.3.6.1.2.1.25.2.1.4";
    |               ^^^^^^^^^^

warning: constant `FLASH` is never used
   --> src/profiles/standard_oids.rs:186:15
    |
186 |     pub const FLASH: &str           = "1.3.6.1.2.1.25.2.1.7";
    |               ^^^^^

warning: constant `NETWORK_DISK` is never used
   --> src/profiles/standard_oids.rs:187:15
    |
187 |     pub const NETWORK_DISK: &str    = "1.3.6.1.2.1.25.2.1.10";
    |               ^^^^^^^^^^^^

warning: method `as_str` is never used
  --> src/snmp/mod.rs:63:12
   |
55 | impl SnmpVersion {
   | ---------------- method in this implementation
...
63 |     pub fn as_str(&self) -> &str {
   |            ^^^^^^

warning: fields `vendor` and `description` are never read
  --> src/snmp/mod.rs:82:9
   |
78 | pub struct SnmpClient {
   |            ---------- fields in this struct
...
82 |     pub vendor: String,
   |         ^^^^^^
83 |     pub description: String,
   |         ^^^^^^^^^^^

warning: method `connection_info` is never used
   --> src/snmp/mod.rs:174:12
    |
105 | impl SnmpClient {
    | --------------- method in this implementation
...
174 |     pub fn connection_info(&self) -> serde_json::Value {
    |            ^^^^^^^^^^^^^^^

warning: constant `TAG_SET_REQUEST` is never used
  --> src/snmp/ber.rs:33:11
   |
33 | pub const TAG_SET_REQUEST: u8 = 0xa3;
   |           ^^^^^^^^^^^^^^^

warning: constant `TAG_PLAIN_TEXT` is never used
  --> src/snmp/ber.rs:43:11
   |
43 | pub const TAG_PLAIN_TEXT: u8 = 0xa0;  // msgData plaintext
   |           ^^^^^^^^^^^^^^

warning: constant `TAG_ENCRYPTED` is never used
  --> src/snmp/ber.rs:44:11
   |
44 | pub const TAG_ENCRYPTED: u8 = 0xa1;   // msgData encrypted
   |           ^^^^^^^^^^^^^

warning: function `encode_uint` is never used
   --> src/snmp/ber.rs:145:8
    |
145 | pub fn encode_uint(value: u64) -> Vec<u8> {
    |        ^^^^^^^^^^^

warning: field `request_id` is never read
   --> src/snmp/ber.rs:389:9
    |
388 | pub struct SnmpPdu {
    |            ------- field in this struct
389 |     pub request_id: i32,
    |         ^^^^^^^^^^
    |
    = note: `SnmpPdu` has a derived impl for the trait `Debug`, but this is intentionally ignored during dead code analysis

warning: methods `as_f64`, `has_data`, and `to_json` are never used
   --> src/snmp/types.rs:70:12
    |
 42 | impl SnmpValue {
    | -------------- methods in this implementation
...
 70 |     pub fn as_f64(&self) -> Option<f64> {
    |            ^^^^^^
...
116 |     pub fn has_data(&self) -> bool {
    |            ^^^^^^^^
...
127 |     pub fn to_json(&self) -> serde_json::Value {
    |            ^^^^^^^

warning: method `has_data` is never used
   --> src/snmp/types.rs:194:12
    |
173 | impl SnmpResult {
    | --------------- method in this implementation
...
194 |     pub fn has_data(&self) -> bool {
    |            ^^^^^^^^

warning: method `key_length` is never used
  --> src/snmp/v3.rs:62:12
   |
43 | impl AuthProtocol {
   | ----------------- method in this implementation
...
62 |     pub fn key_length(&self) -> usize {
   |            ^^^^^^^^^^

warning: fields `username`, `priv_protocol`, `engine_id`, `engine_boots`, and `engine_time` are never read
   --> src/snmp/v3.rs:396:9
    |
395 | pub struct UsmSecurityParams {
    |            ----------------- fields in this struct
396 |     pub username: String,
    |         ^^^^^^^^
...
399 |     pub priv_protocol: PrivProtocol,
    |         ^^^^^^^^^^^^^
400 |     pub priv_key_localized: Vec<u8>,
401 |     pub engine_id: Vec<u8>,
    |         ^^^^^^^^^
402 |     pub engine_boots: u32,
    |         ^^^^^^^^^^^^
403 |     pub engine_time: u32,
    |         ^^^^^^^^^^^
    |
    = note: `UsmSecurityParams` has derived impls for the traits `Clone` and `Debug`, but these are intentionally ignored during dead code analysis

warning: function `mb_to_gb` is never used
  --> src/utils/conversions.rs:12:8
   |
12 | pub fn mb_to_gb(mb: f64) -> f64 {
   |        ^^^^^^^^

warning: function `safe_division` is never used
  --> src/utils/conversions.rs:22:8
   |
22 | pub fn safe_division(numerator: f64, denominator: f64) -> f64 {
   |        ^^^^^^^^^^^^^

warning: function `safe_int` is never used
  --> src/utils/conversions.rs:63:8
   |
63 | pub fn safe_int(value: &str) -> i64 {
   |        ^^^^^^^^

warning: function `safe_float` is never used
  --> src/utils/conversions.rs:68:8
   |
68 | pub fn safe_float(value: &str) -> f64 {
   |        ^^^^^^^^^^

warning: function `json_to_i64` is never used
  --> src/utils/conversions.rs:73:8
   |
73 | pub fn json_to_i64(v: &serde_json::Value) -> i64 {
   |        ^^^^^^^^^^^

warning: function `json_to_f64` is never used
  --> src/utils/conversions.rs:82:8
   |
82 | pub fn json_to_f64(v: &serde_json::Value) -> f64 {
   |        ^^^^^^^^^^^

warning: function `bps_to_mbps` is never used
  --> src/utils/conversions.rs:91:8
   |
91 | pub fn bps_to_mbps(bps: u64) -> f64 {
   |        ^^^^^^^^^^^

warning: function `format_bytes` is never used
  --> src/utils/conversions.rs:96:8
   |
96 | pub fn format_bytes(bytes: u64) -> String {
   |        ^^^^^^^^^^^^

warning: function `interface_error_rate` is never used
   --> src/utils/conversions.rs:109:8
    |
109 | pub fn interface_error_rate(errors: u64, total_packets: u64) -> f64 {
    |        ^^^^^^^^^^^^^^^^^^^^

warning: function `now_iso_utc` is never used
  --> src/utils/helpers.rs:15:8
   |
15 | pub fn now_iso_utc() -> String {
   |        ^^^^^^^^^^^

warning: function `print_simple` is never used
  --> src/utils/helpers.rs:22:8
   |
22 | pub fn print_simple(msg: &str) {
   |        ^^^^^^^^^^^^

warning: function `json_str` is never used
  --> src/utils/helpers.rs:27:8
   |
27 | pub fn json_str(v: &serde_json::Value, key: &str) -> String {
   |        ^^^^^^^^

warning: function `json_i64` is never used
  --> src/utils/helpers.rs:35:8
   |
35 | pub fn json_i64(v: &serde_json::Value, key: &str) -> i64 {
   |        ^^^^^^^^

warning: function `json_f64` is never used
  --> src/utils/helpers.rs:42:8
   |
42 | pub fn json_f64(v: &serde_json::Value, key: &str) -> f64 {
   |        ^^^^^^^^

warning: function `json_set` is never used
  --> src/utils/helpers.rs:49:8
   |
49 | pub fn json_set(obj: &mut serde_json::Value, key: &str, value: serde_json::Value) {
   |        ^^^^^^^^

warning: function `json_merge` is never used
  --> src/utils/helpers.rs:56:8
   |
56 | pub fn json_merge(base: &mut serde_json::Value, extra: serde_json::Value) {
   |        ^^^^^^^^^^