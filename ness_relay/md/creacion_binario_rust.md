# Revision del metodo de construccion del binario relay

Fecha de revision: 2026-05-18

## Objetivo

Validar si al ejecutar el script `./build_relay.sh` para una arquitectura especifica (`x86_64` o `aarch64`) tambien se genera adicionalmente un binario base llamado `ness-relay`.

## Hallazgo principal

Si, lo que reportaste es correcto.

El script no genera un solo archivo por arquitectura; siempre copia el mismo binario compilado en DOS nombres dentro de `dist/`:

1. `ness-relay` (alias/base)
2. `ness-relay-<arquitectura>`

Esto ocurre por estas dos lineas del script:

- Archivo: `agentes/ness_relay/rust/ness_relay_v2.0.0/build_relay.sh`
- Lineas relevantes:

```bash
cp "${BINARY_PATH}" "${OUTPUT_DIR}/${BINARY_NAME}"
cp "${BINARY_PATH}" "${OUTPUT_DIR}/${ARCH_BINARY_NAME}"
```

Donde:

- `BINARY_NAME="ness-relay"`
- `ARCH_BINARY_NAME="ness-relay-${ARCH}"`

## Evidencia de ejecucion real

### Caso 1: build x86_64

Comando ejecutado:

```bash
./build_relay.sh --arch x86_64 --release --yes
```

Estado final de `dist/` despues del build:

- `ness-relay`
- `ness-relay-aarch64` (ya existente de build previo)
- `ness-relay-x86_64`

Adicionalmente, `ness-relay` quedo con el mismo tamano/fecha que `ness-relay-x86_64`, confirmando que el alias fue sobreescrito con la ultima arquitectura compilada.

### Caso 2: build aarch64

Comando ejecutado:

```bash
./build_relay.sh --arch aarch64 --release --yes
```

Estado final de `dist/` despues del build:

- `ness-relay`
- `ness-relay-aarch64`
- `ness-relay-x86_64`

En este caso, `ness-relay` quedo con el mismo tamano/fecha que `ness-relay-aarch64`, confirmando nuevamente que el alias `ness-relay` siempre se actualiza con el ultimo build ejecutado.

## Conclusion

Tu observacion era totalmente valida para la version anterior del script: por diseno anterior, cada build generaba dos copias del mismo binario para la arquitectura seleccionada.

- Una con nombre fijo (`ness-relay`)
- Otra con sufijo de arquitectura (`ness-relay-x86_64` o `ness-relay-aarch64`)

Eso fue la causa de la confusion inicial y de los archivos duplicados en `dist/`.

## Cambio aplicado al metodo de build

Se actualizo el script para que ahora deje un solo artefacto por ejecucion:

- `./build_relay.sh --arch x86_64 --release` produce solo `dist/ness-relay-x86_64`
- `./build_relay.sh --arch aarch64 --release` produce solo `dist/ness-relay-aarch64`

El binario con nombre generico `ness-relay` ya no se copia a `dist/` desde este script de build.
Ademas, si existia un `ness-relay` previo en `dist/`, el script lo elimina antes de copiar el nuevo artefacto.

### Efecto practico

1. Se elimina la ambiguedad entre alias base y binario por arquitectura.
2. Cada ejecucion del build deja un unico artefacto claramente identificable.
3. El instalador sigue pudiendo trabajar con el binario por arquitectura, ya que puede resolver el ejecutable correcto desde `dist/`.

## Cambio aplicado a la instalacion

Tambien se ajusto `install_relay.sh` para que la instalacion copie y ejecute el binario real por arquitectura dentro de `/opt/ness_relay/executables/`.

### Comportamiento nuevo

1. Si el host es `x86_64`, el instalador deja `ness-relay-x86_64` en `executables/`.
2. Si el host es `aarch64`, el instalador deja `ness-relay-aarch64` en `executables/`.
3. El script `run_relay.sh` generado por la instalacion invoca ese mismo nombre, no un alias generico.
4. La validacion interna `--verify-setup` tambien usa el binario instalado por arquitectura.
5. En una actualizacion, el instalador guarda backup de `ness-relay`, `ness-relay-x86_64` y `ness-relay-aarch64` si alguno existe.

### Impacto

1. La carpeta `executables/` queda consistente con el artefacto del build.
2. Se reduce la confusion entre binarios antiguos y nuevos.
3. El cron sigue funcionando porque ahora apunta al ejecutable correcto instalado en el sistema.




## Registro de comandos y parametros disponibles en build_relay.sh

Esta seccion responde especificamente tu duda sobre `--yes` y documenta todas las opciones que soporta el script.

### Que hace el parametro --yes

`--yes` (o su alias corto `-y`) activa el modo no interactivo (`YES_MODE=true`).

En la practica significa que, cuando el script necesita confirmar instalaciones (por ejemplo Rust o toolchains musl), no te pregunta en consola y asume automaticamente respuesta afirmativa.

Esto evita prompts tipo:

- "Desea instalar Rust? (Y/n)"
- "Desea instalar musl-tools? (Y/n)"

Por eso en las pruebas use `--yes`: permite ejecutar/validar el flujo completo sin quedar esperando entrada manual.

### Sintaxis general

```bash
./build_relay.sh [--arch x86_64|aarch64] [--release|--debug] [--yes|-y]
```

### Parametros soportados

1. `--arch x86_64`
	- Define la arquitectura objetivo en x86_64.
	- Es el valor por defecto si no pasas `--arch`.
	- Target interno: `x86_64-unknown-linux-musl`.

2. `--arch aarch64`
	- Define la arquitectura objetivo en ARM64.
	- Target interno: `aarch64-unknown-linux-musl`.

3. `--arch arm64`
	- Alias aceptado por el parser.
	- Internamente el script lo normaliza a `aarch64`.

4. `--release`
	- Compila en modo produccion (optimizaciones activas).
	- Es el perfil por defecto.

5. `--debug`
	- Compila en modo debug (mas rapido para pruebas locales, con simbolos de depuracion).

6. `--yes`
	- Modo no interactivo: acepta automaticamente preguntas de instalacion.

7. `-y`
	- Alias corto de `--yes`.

### Comandos practicos disponibles

1. Build por defecto (equivalente a x86_64 release):

```bash
./build_relay.sh
```

2. Build x86_64 release (tu comando actual):

```bash
./build_relay.sh --arch x86_64 --release
```

3. Build aarch64 release (tu comando actual):

```bash
./build_relay.sh --arch aarch64 --release
```

4. Build x86_64 debug:

```bash
./build_relay.sh --arch x86_64 --debug
```

5. Build aarch64 debug:

```bash
./build_relay.sh --arch aarch64 --debug
```

6. Build x86_64 release sin prompts (modo CI/no interactivo):

```bash
./build_relay.sh --arch x86_64 --release --yes
```

7. Build aarch64 release sin prompts (modo CI/no interactivo):

```bash
./build_relay.sh --arch aarch64 --release --yes
```

8. Build usando alias arm64:

```bash
./build_relay.sh --arch arm64 --release
```

### Notas de comportamiento utiles

1. `--release` y `--debug` son excluyentes en intencion; si pasas ambos, aplica el ultimo que aparezca en la linea de comando.
2. Si pasas una opcion no soportada, el script termina con error y muestra la ayuda de uso.
3. El binario compilado ahora se copia solo a un nombre en `dist/`: `ness-relay-<arquitectura>`.






SISTEMA DE AUTO-ACTUALIZACION DEL AGENTE RELAY

root@relay-server:/home/tecnologia/relay_rust# ls
clean_relay.sh  install_relay.sh  ness-relay-x86_64
root@relay-server:/home/tecnologia/relay_rust# ./ness-relay-x86_64 --version
NESS Relay Multi-Vendor v2.0.0 (ness-relay)
root@relay-server:/home/tecnologia/relay_rust#

