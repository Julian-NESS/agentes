// =============================================================================
// NESS Relay v2.5.1 — CLI de credenciales (subcomando del binario principal)
// =============================================================================
//
// Phase 2.5.1: toda la lógica de cifrado de credenciales ahora vive como
// subcomandos del binario `ness-relay` (no como binario separado).
//
// Uso típico:
//   sudo ness-relay credential migrate-plaintext
//   sudo ness-relay credential set NESS_SSH_PASSWORD_fortinet_1
//   sudo ness-relay credential status
//   sudo ness-relay credential list
//
// El instalador `install_relay.sh` invoca:
//   ness-relay credential encrypt-field <device> <field>    (lee valor por stdin)
//   ness-relay credential set <env_var>                    (lee valor por stdin)
// =============================================================================

use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::secrets;
use crate::secrets::migration::migrate_plaintext_config;

#[derive(Subcommand, Debug)]
pub enum CredentialCmd {
    /// Cifra los campos sensibles en `connection.config` (migración
    /// retroactiva desde instalaciones v2.4.0 en texto plano).
    MigratePlaintext {
        /// Ruta al connection.config (default: $NESS_DEVICES_FILE o
        /// /opt/ness_relay/configs/connection.config).
        #[arg(long, short = 'c')]
        config: Option<PathBuf>,
        /// No pedir confirmación de cada pass (asume sí).
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Verifica que un campo concreto (device_id.field) descifra OK.
    TestCred {
        /// device_id, ej: "fortinet_1"
        device: String,
        /// field, ej: "v3_auth_password" o "v3_priv_password"
        field: String,
    },
    /// Muestra los campos sensibles de un config como "(cifrado)" sin
    /// revelar plaintext. Útil para debug.
    ShowConfig {
        #[arg(long, short = 'c')]
        config: Option<PathBuf>,
    },
    /// Cifra un valor SSH/secret y lo guarda en /etc/ness_relay/secrets.enc.
    /// Lee el valor por stdin (no por argumento, para que la pass no aparezca
    /// en `ps`/`history`).
    Set {
        /// Nombre del env var (ej: NESS_SSH_PASSWORD_fortinet_1).
        env_var: String,
        /// No pedir confirmación de la pass (asume stdin confiable).
        /// Usado por `install_relay.sh` en modo no-interactivo.
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Cifra un valor de un campo del connection.config (ej: v3_auth_password)
    /// y lo imprime por stdout. Usado por install_relay.sh para cifrar
    /// las pass SNMPv3 al momento de la instalación.
    EncryptField {
        /// device_id (ej: fortinet_1)
        device: String,
        /// field (ej: v3_auth_password)
        field: String,
    },
    /// Descifra un valor (leído por stdin como token $enc$2$...) con el
    /// AAD = "<device>|<field>" e imprime el plaintext por stdout.
    /// Usado por install_relay.sh para re-cifrar campos post-probe.
    DecryptField {
        /// device_id (ej: fortinet_1)
        device: String,
        /// field (ej: v3_auth_password)
        field: String,
    },
    /// Lista los env vars guardados en secrets.enc (sin valores).
    List,
    /// Diagnóstico del vault (machine-id, .salt, .seed).
    Status,
    /// Elimina todos los secretos cifrados (NO toca connection.config).
    /// Útil para `set` desde cero durante una rotación.
    Clear,
}

/// Punto de entrada invocado desde `main()` cuando se detecta el
/// subcomando `credential ...`. Retorna un ExitCode para preservar
/// la semántica de códigos de salida del binario.
pub fn run(cmd: CredentialCmd) -> ExitCode {
    let result: Result<ExitCode> = match cmd {
        CredentialCmd::MigratePlaintext { config, yes } => cmd_migrate(config, yes),
        CredentialCmd::TestCred { device, field } => cmd_test(&device, &field),
        CredentialCmd::ShowConfig { config } => cmd_show(config),
        CredentialCmd::Set { env_var, yes } => cmd_set(&env_var, yes),
        CredentialCmd::EncryptField { device, field } => cmd_encrypt_field(&device, &field),
        CredentialCmd::DecryptField { device, field } => cmd_decrypt_field(&device, &field),
        CredentialCmd::List => cmd_list(),
        CredentialCmd::Status => cmd_status(),
        CredentialCmd::Clear => cmd_clear(),
    };
    match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("[ERROR] {e:#}");
            ExitCode::from(1)
        }
    }
}

// -----------------------------------------------------------------------------
// Subcomandos
// -----------------------------------------------------------------------------

fn cmd_migrate(config: Option<PathBuf>, yes: bool) -> Result<ExitCode> {
    let config_path = config
        .unwrap_or_else(secrets::connection_config_default);
    if !config_path.exists() {
        eprintln!("No se encontró connection.config en {}", config_path.display());
        eprintln!("Use --config /ruta/al/config");
        return Ok(ExitCode::from(2));
    }
    eprintln!("→ Migrando credenciales en {}", config_path.display());

    // Para tests de CI / no-interactivo: el operador puede exportar
    // NESS_RELAY_CRED_INPUT=<archivo> con un password por línea. Útil
    // para automatizar migraciones.
    let stdin_is_pipe = !io::stdin().is_terminal();

    let master_key = secrets::master_key()
        .context("No se pudo derivar la clave maestra. ¿Ejecutar install_relay.sh?")?;

    let report = migrate_plaintext_config(
        &master_key,
        &config_path,
        None,
        |field, aad| {
            let result: Result<Option<String>> = (|| {
                if yes || stdin_is_pipe {
                    let line = read_line_stdin()
                        .map_err(|e| anyhow::anyhow!("leyendo stdin: {e}"))?;
                    if line.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(line));
                }
                eprintln!();
                eprint!("Ingrese el valor para {aad} (Enter vacío = skip): ");
                io::stderr().flush().ok();
                let line = read_line_stdin()
                    .map_err(|e| anyhow::anyhow!("leyendo stdin: {e}"))?;
                if line.is_empty() {
                    eprintln!("  → skip");
                    return Ok(None);
                }
                eprint!("Confirme el valor para {field}: ");
                io::stderr().flush().ok();
                let confirm = read_line_stdin()
                    .map_err(|e| anyhow::anyhow!("leyendo stdin: {e}"))?;
                if confirm != line {
                    eprintln!("  → confirmación no coincide — skip");
                    return Ok(None);
                }
                Ok(Some(line))
            })();
            result.map_err(|e| {
                use crate::secrets::migration::MigrationError;
                MigrationError::Malformed(e.to_string())
            })
        },
        |_field| Ok(true),
    )?;

    eprintln!();
    eprintln!("✓ Migración completada:");
    eprintln!("  Dispositivos escaneados:  {}", report.devices_scanned);
    eprintln!("  Campos cifrados:          {}", report.fields_migrated);
    eprintln!("  Campos ya cifrados (skip): {}", report.fields_skipped_existing);
    if let Some(p) = &report.backup_path {
        eprintln!("  Backup del original:     {}", p.display());
    }
    eprintln!();
    eprintln!("IMPORTANTE: ya NO exporte NESS_SSH_PASSWORD_* como variable de entorno");
    eprintln!("El agente descifra on-demand desde /etc/ness_relay/secrets.enc.");
    Ok(ExitCode::SUCCESS)
}

fn cmd_test(device: &str, field: &str) -> Result<ExitCode> {
    let config_path = secrets::connection_config_default();
    if !config_path.exists() {
        eprintln!("No se encontró connection.config en {}", config_path.display());
        return Ok(ExitCode::from(2));
    }
    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("leyendo {}", config_path.display()))?;
    let key = format!("{device}_{field}");
    let val = find_kv_value(&content, &key)
        .ok_or_else(|| anyhow::anyhow!("clave '{key}' no encontrada en {}", config_path.display()))?;
    if val.is_empty() {
        eprintln!("[WARN] '{key}' existe pero está VACÍA");
        return Ok(ExitCode::from(2));
    }
    if !secrets::is_encrypted_token(val.as_str()) {
        eprintln!("[INFO] '{key}' está en texto plano (no es un token $enc$).");
        eprintln!("        Use `ness-relay credential migrate-plaintext` para cifrar.");
        return Ok(ExitCode::SUCCESS);
    }
    let aad = format!("{device}|{field}");
    let master_key = secrets::master_key()?;
    match secrets::decrypt_str(&master_key, val.as_str(), aad.as_bytes()) {
        Ok(plain) => {
            println!("[OK]   {key}: descifra correctamente (len = {})", plain.len());
            println!("       (el valor no se imprime por seguridad)");
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            eprintln!("[FAIL] {key}: {e}");
            eprintln!("       AAD usado: {aad:?}");
            eprintln!("       ¿Restauraste el config de otro host? ¿se perdió .salt?");
            Ok(ExitCode::from(1))
        }
    }
}

fn cmd_show(config: Option<PathBuf>) -> Result<ExitCode> {
    let config_path = config.unwrap_or_else(secrets::connection_config_default);
    if !config_path.exists() {
        eprintln!("No se encontró connection.config en {}", config_path.display());
        return Ok(ExitCode::from(2));
    }
    let content = std::fs::read_to_string(&config_path)?;
    for line in content.lines() {
        if let Some((k, v)) = parse_kv(line) {
            if secrets::is_encrypted_token(v.as_str()) {
                println!("{k} = $enc$2$... ({} bytes cifrados)", v.len());
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_set(env_var: &str, yes: bool) -> Result<ExitCode> {
    if !is_valid_env_var_name(env_var) {
        eprintln!("[ERROR] '{env_var}' no es un nombre de env var válido");
        return Ok(ExitCode::from(2));
    }
    let value: String = if yes {
        // Modo no-interactivo (usado por install_relay.sh): leer la pass
        // por stdin y NO pedir confirmación. El operador tipea la pass
        // una vez y el binario la cifra inmediatamente.
        let mut s = String::new();
        io::stdin().lock().read_line(&mut s)?;
        s.trim_end_matches(['\n', '\r']).to_string()
    } else {
        // Modo interactivo: prompt + confirmación.
        eprint!("Ingrese el valor para {env_var}: ");
        io::stderr().flush().ok();
        let v = read_secret_stdin()?;
        eprint!("Confirme: ");
        io::stderr().flush().ok();
        let confirm = read_secret_stdin()?;
        if v != confirm {
            eprintln!("[ERROR] confirmación no coincide — no se guardó");
            return Ok(ExitCode::from(1));
        }
        v
    };
    if value.is_empty() {
        eprintln!("[ERROR] valor vacío — no se guardó");
        return Ok(ExitCode::from(2));
    }
    let master_key = secrets::master_key()?;
    let path = secrets::secrets_file();

    let mut map = secrets::load_env_file_decrypted(&master_key, &path)?;
    map.insert(env_var.to_string(), value);
    secrets::save_env_file_encrypted(&master_key, &path, &map)?;
    eprintln!("[OK] '{env_var}' guardado en {}", path.display());
    Ok(ExitCode::SUCCESS)
}

fn cmd_encrypt_field(device: &str, field: &str) -> Result<ExitCode> {
    // Lee el valor por stdin (sin prompt para que sea invocable desde scripts).
    let mut value = String::new();
    io::stdin().lock().read_line(&mut value)?;
    let value = value.trim_end_matches(['\n', '\r']).to_string();
    if value.is_empty() {
        eprintln!("[ERROR] valor vacío (se esperaba por stdin)");
        return Ok(ExitCode::from(2));
    }
    let master_key = secrets::master_key()?;
    let aad = format!("{device}|{field}");
    let token = secrets::encrypt_str(&master_key, &value, aad.as_bytes())?;
    // Imprimir SOLO el token por stdout (parseable por scripts).
    println!("{token}");
    Ok(ExitCode::SUCCESS)
}

/// Descifra un token $enc$2$... (leído por stdin) e imprime el plaintext
/// por stdout. Usado por `install_relay.sh` para re-cifrar campos después
/// del probe (cuando el device_id cambió de `generic_1` a `fortinet_1`).
///
/// Códigos de salida:
/// - 0: descifrado OK, plaintext en stdout
/// - 1: error de descifrado (AAD mismatch, tampering, etc.)
/// - 2: input inválido (no es token, vacío, etc.)
fn cmd_decrypt_field(device: &str, field: &str) -> Result<ExitCode> {
    let mut token = String::new();
    io::stdin().lock().read_line(&mut token)?;
    let token = token.trim_end_matches(['\n', '\r']).to_string();
    if token.is_empty() {
        eprintln!("[ERROR] token vacío (se esperaba por stdin)");
        return Ok(ExitCode::from(2));
    }
    if !secrets::is_encrypted_token(&token) {
        eprintln!("[ERROR] no es un token $enc$2$...");
        return Ok(ExitCode::from(2));
    }
    let master_key = secrets::master_key()?;
    let aad = format!("{device}|{field}");
    match secrets::decrypt_str(&master_key, &token, aad.as_bytes()) {
        Ok(plain) => {
            // Imprimir SOLO el plaintext por stdout (parseable por scripts).
            // `plain` es Zeroizing<String> — accedemos al &str interno
            // para el println sin que se filtre el wrapper.
            println!("{}", &*plain);
            Ok(ExitCode::SUCCESS)
        }
        Err(e) => {
            eprintln!("[ERROR] {e}");
            Ok(ExitCode::from(1))
        }
    }
}

fn cmd_list() -> Result<ExitCode> {
    let master_key = secrets::master_key()?;
    let path = secrets::secrets_file();
    if !path.exists() {
        eprintln!("(secrets.enc no existe — no hay credenciales guardadas)");
        return Ok(ExitCode::SUCCESS);
    }
    let map = secrets::load_env_file_decrypted(&master_key, &path)?;
    println!("Env vars en {} ({}):", path.display(), map.len());
    let mut keys: Vec<_> = map.keys().collect();
    keys.sort();
    for k in keys {
        println!("  {k}  (len = {})", map[k].len());
    }
    Ok(ExitCode::SUCCESS)
}

fn cmd_status() -> Result<ExitCode> {
    let paths = secrets::vault_paths();
    let mid_path = "/etc/machine-id";
    let mid_exists = std::path::Path::new(mid_path).exists();
    let mid_sample = if mid_exists {
        std::fs::read_to_string(mid_path)
            .ok()
            .map(|s| s.trim().chars().take(16).collect::<String>())
            .unwrap_or_default()
    } else {
        String::new()
    };
    println!("NESS Relay — vault status");
    println!("  Root:                 {}", paths.root.display());
    println!("  Salt existe:           {}", paths.salt.exists());
    println!("  Seed existe (fb):      {}", paths.seed.exists());
    println!("  secrets.enc existe:   {}", secrets::secrets_file().exists());
    println!("  /etc/machine-id:       {} ({})", if mid_exists { "OK" } else { "AUSENTE" }, mid_sample);
    let mk = secrets::master_key()?;
    println!("  Master key derivada:   OK ({} bytes zeroized)", mk.len());
    Ok(ExitCode::SUCCESS)
}

fn cmd_clear() -> Result<ExitCode> {
    let path = secrets::secrets_file();
    if !path.exists() {
        eprintln!("secrets.enc no existe — nada que borrar");
        return Ok(ExitCode::SUCCESS);
    }
    eprint!("¿Seguro que quieres ELIMINAR {}? (escribe 'si' para confirmar): ", path.display());
    io::stderr().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    if input.trim() != "si" {
        eprintln!("Cancelado");
        return Ok(ExitCode::SUCCESS);
    }
    std::fs::remove_file(&path)?;
    eprintln!("[OK] {} eliminado", path.display());
    Ok(ExitCode::SUCCESS)
}

// -----------------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------------

fn read_line_stdin() -> Result<String> {
    let mut s = String::new();
    io::stdin().lock().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

/// Lee stdin sin eco (TTY). En pipes (CI) cae a lectura normal.
/// Usa `stty -echo` para deshabilitar el echo temporalmente (best-effort).
fn read_secret_stdin() -> Result<String> {
    if io::stdin().is_terminal() {
        let _ = std::process::Command::new("stty")
            .args(["-echo"])
            .status();
    }
    let r = read_line_stdin();
    if io::stdin().is_terminal() {
        let _ = std::process::Command::new("stty")
            .args(["echo"])
            .status();
        eprintln!();
    }
    r
}

fn find_kv_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        if let Some((k, v)) = parse_kv(line) {
            if k == key { return Some(v); }
        }
    }
    None
}

fn parse_kv(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';')
        || trimmed.starts_with('[') {
        return None;
    }
    let no_comment = if let Some(idx) = trimmed.find(" #") { &trimmed[..idx] }
        else if let Some(idx) = trimmed.find(" ;") { &trimmed[..idx] }
        else { trimmed };
    let mut parts = no_comment.trim_end().splitn(2, '=');
    let k = parts.next()?.trim().to_string();
    let v = parts.next()?.trim().to_string();
    if k.is_empty() || v.is_empty() { return None; }
    if !k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') { return None; }
    Some((k, v))
}

fn is_valid_env_var_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().map_or(false, |c| c == '_' || c.is_ascii_alphabetic())
        && s.chars().all(|c| c == '_' || c.is_ascii_alphanumeric())
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_env_var_name_accepts_typical() {
        assert!(is_valid_env_var_name("NESS_SSH_PASSWORD_fortinet_1"));
        assert!(is_valid_env_var_name("_PRIVATE"));
        assert!(is_valid_env_var_name("A1"));
        assert!(!is_valid_env_var_name(""));
        assert!(!is_valid_env_var_name("1ABC"));
        assert!(!is_valid_env_var_name("ABC-X"));
        assert!(!is_valid_env_var_name("ABC.DEF"));
    }

    #[test]
    fn parse_kv_basic() {
        let (k, v) = parse_kv("fortinet_1_v3_auth_password = $enc$2$XXXX").unwrap();
        assert_eq!(k, "fortinet_1_v3_auth_password");
        assert_eq!(v, "$enc$2$XXXX");
    }

    #[test]
    fn parse_kv_skips_comments() {
        assert!(parse_kv("# comentario").is_none());
        assert!(parse_kv("; comentario").is_none());
        assert!(parse_kv("[seccion]").is_none());
        assert!(parse_kv("").is_none());
    }
}
