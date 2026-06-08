#!/usr/bin/env bash
# install-formal-tools.sh - install required formal-methods tools.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "${REPO_ROOT}"

if [ -t 1 ] && command -v tput >/dev/null 2>&1; then
    GRN="$(tput setaf 2)"
    YLW="$(tput setaf 3)"
    RED="$(tput setaf 1)"
    RST="$(tput sgr0)"
else
    GRN=""
    YLW=""
    RED=""
    RST=""
fi

info() { echo "${YLW}[formal-tools]${RST} $*"; }
ok() { echo "${GRN}[PASS]${RST} $*"; }
fail() { echo "${RED}[FAIL]${RST} $*" >&2; exit 1; }

LEAN_TOOLCHAIN="$(tr -d '[:space:]' < lean-toolchain)"
TLA_VERSION="v1.7.4"
TLA_URL="https://github.com/tlaplus/tlaplus/releases/download/${TLA_VERSION}/tla2tools.jar"
TOOLS_DIR="${REPO_ROOT}/.tools"
BIN_DIR="${TOOLS_DIR}/bin"
TLA_JAR="${TOOLS_DIR}/tla2tools-${TLA_VERSION}.jar"
TLA_CURRENT="${TOOLS_DIR}/tla2tools.jar"

echo "=== Loom formal tool installer ==="
echo "Repository: ${REPO_ROOT}"
echo "Lean toolchain: ${LEAN_TOOLCHAIN}"
echo "TLC version: ${TLA_VERSION}"
echo ""

if ! command -v curl >/dev/null 2>&1; then
    fail "curl is required to install formal tools"
fi

export PATH="${HOME}/.elan/bin:${PATH}"

if ! command -v elan >/dev/null 2>&1; then
    info "Installing elan..."
    curl -fsSL https://raw.githubusercontent.com/leanprover/elan/master/elan-init.sh \
        | sh -s -- -y --default-toolchain none
fi

if ! command -v elan >/dev/null 2>&1; then
    fail "elan was not found after installation; add ${HOME}/.elan/bin to PATH"
fi

if elan toolchain list | grep -Fq "${LEAN_TOOLCHAIN}"; then
    ok "Lean toolchain ${LEAN_TOOLCHAIN} already installed"
else
    info "Installing Lean toolchain ${LEAN_TOOLCHAIN}..."
    elan toolchain install "${LEAN_TOOLCHAIN}"
    ok "elan toolchain install ${LEAN_TOOLCHAIN}"
fi

info "Checking Lean..."
lean --version
lake --version
lean formal/lean/LoomCore.lean
ok "Lean scaffold check"

mkdir -p "${BIN_DIR}"

if [ ! -f "${TLA_JAR}" ]; then
    info "Downloading TLC ${TLA_VERSION}..."
    curl -fL "${TLA_URL}" -o "${TLA_JAR}"
fi
ln -sf "$(basename "${TLA_JAR}")" "${TLA_CURRENT}"

cat > "${BIN_DIR}/tlc" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
JAR="${SCRIPT_DIR}/../tla2tools.jar"
if command -v mise >/dev/null 2>&1 && [ -f "${REPO_ROOT}/.mise.toml" ]; then
    exec mise exec -- java -jar "${JAR}" "$@"
fi
exec java -jar "${JAR}" "$@"
EOF
chmod +x "${BIN_DIR}/tlc"

export PATH="${BIN_DIR}:${PATH}"

if command -v java >/dev/null 2>&1; then
    JAVA_CHECK=(java -version)
elif command -v mise >/dev/null 2>&1; then
    JAVA_CHECK=(mise exec -- java -version)
else
    fail "java is required for TLC; run mise install or install Java 21"
fi

info "Checking TLC..."
"${JAVA_CHECK[@]}" >/dev/null
tlc -config specs/tla/LoomVerifierPipeline.cfg specs/tla/LoomVerifierPipeline.tla
ok "TLC lifecycle model check"

echo ""
echo "${GRN}=== Formal tools installed ===${RST}"
