#!/usr/bin/env python3
"""L2Core AST sync checklist (Phase 48 P5).

Performs a lightweight, best-effort comparison of the AST inventories
across three artifacts:
  1. K spec        — contrib/kloom/src/kloom.k
  2. Rust core     — crates/loom-core/src/l2_core.rs
  3. Lean formal   — formal/lean/LoomCore.lean

Rather than full grammar parsing (which is fragile across three radically
different syntaxes), the script searches for *presence/absence* of key
constructors/operators in each artifact.  It reports:
  ✓  found in all three
  ⚠  missing in one or two artifacts

Exit 0 if no critical divergences, 1 otherwise.
"""

import re
import sys
from pathlib import Path
from dataclasses import dataclass
from typing import Dict, List, Set

ROOT = Path(__file__).resolve().parent.parent


def read_text(rel_path: str) -> str:
    return (ROOT / rel_path).read_text()


# ---------------------------------------------------------------------------
# Simple presence detectors
# ---------------------------------------------------------------------------

def k_has(token: str, text: str) -> bool:
    # K productions are quoted or bare words inside syntax blocks
    return token in text


def rust_has(token: str, text: str) -> bool:
    # Rust enum variants are PascalCase
    return token in text


def lean_has(token: str, text: str) -> bool:
    # Lean constructors are camelCase after a pipe
    return token in text


# ---------------------------------------------------------------------------
# Check items
# ---------------------------------------------------------------------------

@dataclass
class CheckItem:
    name: str
    k_tokens: List[str]
    rust_tokens: List[str]
    lean_tokens: List[str]


CHECKS: List[CheckItem] = [
    # ScalarExpr operators
    CheckItem(
        "ScalarExpr::Add",
        k_tokens=["Add"],
        rust_tokens=["Add"],
        lean_tokens=["add"],
    ),
    CheckItem(
        "ScalarExpr::Sub",
        k_tokens=["Sub"],
        rust_tokens=["Sub"],
        lean_tokens=["sub"],
    ),
    CheckItem(
        "ScalarExpr::Mul",
        k_tokens=["Mul"],
        rust_tokens=["Mul"],
        lean_tokens=["mul"],
    ),
    CheckItem(
        "ScalarExpr::Min (Phase 48 P1)",
        k_tokens=["Min"],
        rust_tokens=["Min"],
        lean_tokens=["min"],
    ),
    CheckItem(
        "ScalarExpr::Max (Phase 48 P1)",
        k_tokens=["Max"],
        rust_tokens=["Max"],
        lean_tokens=["max"],
    ),
    CheckItem(
        "ScalarExpr::Eq",
        k_tokens=["Eq"],
        rust_tokens=["Eq"],
        lean_tokens=["eq"],
    ),
    CheckItem(
        "ScalarExpr::Lt",
        k_tokens=["Lt"],
        rust_tokens=["Lt"],
        lean_tokens=["lt"],
    ),
    CheckItem(
        "ScalarExpr::Le",
        k_tokens=["Le"],
        rust_tokens=["Le"],
        lean_tokens=["le"],
    ),
    # Stmt / L2CoreStmt / Stmt constructors
    CheckItem(
        "Stmt::AppendValue",
        k_tokens=["appendValue"],
        rust_tokens=["AppendValue"],
        lean_tokens=["appendValue"],
    ),
    CheckItem(
        "Stmt::AppendNull",
        k_tokens=["appendNull"],
        rust_tokens=["AppendNull"],
        lean_tokens=["appendNull"],
    ),
    CheckItem(
        "Stmt::ForRange",
        k_tokens=["forRange"],
        rust_tokens=["ForRange"],
        lean_tokens=["forRange"],
    ),
    CheckItem(
        "Stmt::CursorLoop",
        k_tokens=["cursorLoop"],
        rust_tokens=["CursorLoop"],
        lean_tokens=["cursorLoop"],
    ),
    CheckItem(
        "Stmt::ReadInput",
        k_tokens=["readInput"],
        rust_tokens=["ReadInput"],
        lean_tokens=["readInput"],
    ),
    CheckItem(
        "Stmt::LetScalar",
        k_tokens=["letScalar"],
        rust_tokens=["LetScalar"],
        lean_tokens=["letScalar"],
    ),
    # ScalarType / L2Ty variants
    CheckItem(
        "Type::Int32",
        k_tokens=["int32"],
        rust_tokens=["Int32"],
        lean_tokens=["int32"],
    ),
    CheckItem(
        "Type::Int64",
        k_tokens=["int64"],
        rust_tokens=["Int64"],
        lean_tokens=["int64"],
    ),
    CheckItem(
        "Type::UInt32",
        k_tokens=["uint32"],
        rust_tokens=["UInt32"],
        lean_tokens=["uint32"],
    ),
    CheckItem(
        "Type::UInt64",
        k_tokens=["uint64"],
        rust_tokens=["UInt64"],
        lean_tokens=["uint64"],
    ),
    CheckItem(
        "Type::Float32",
        k_tokens=["float32"],
        rust_tokens=["Float32"],
        lean_tokens=["float32"],
    ),
    CheckItem(
        "Type::Float64",
        k_tokens=["float64"],
        rust_tokens=["Float64"],
        lean_tokens=["float64"],
    ),
    CheckItem(
        "Type::Bool",
        k_tokens=["boolTy"],
        rust_tokens=["Bool"],
        lean_tokens=["| bool"],
    ),
    # Capability kinds
    CheckItem(
        "Capability::Input",
        k_tokens=['"input"'],
        rust_tokens=["InputSlice"],
        lean_tokens=["input"],
    ),
    CheckItem(
        "Capability::Output",
        k_tokens=['"builder"'],
        rust_tokens=["OutputBuilder"],
        lean_tokens=["output"],
    ),
]


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    kloom = read_text("contrib/kloom/src/kloom.k")
    rust = read_text("crates/loom-core/src/l2_core.rs")
    lean = read_text("formal/lean/LoomCore.lean")

    ok = True
    print("=" * 70)
    print("L2Core AST Sync Checklist  (Phase 48 P5)")
    print("=" * 70)
    print(f"Artifacts:")
    print(f"  K     — contrib/kloom/src/kloom.k")
    print(f"  Rust  — crates/loom-core/src/l2_core.rs")
    print(f"  Lean  — formal/lean/LoomCore.lean")
    print()

    for item in CHECKS:
        k_found = any(k_has(t, kloom) for t in item.k_tokens)
        r_found = any(rust_has(t, rust) for t in item.rust_tokens)
        l_found = any(lean_has(t, lean) for t in item.lean_tokens)

        status = "✓" if (k_found and r_found and l_found) else "⚠"
        if not (k_found and r_found and l_found):
            ok = False

        missing = []
        if not k_found:
            missing.append("K")
        if not r_found:
            missing.append("Rust")
        if not l_found:
            missing.append("Lean")

        if missing:
            print(f"{status} {item.name:45s}  missing: {', '.join(missing)}")
        else:
            print(f"{status} {item.name:45s}")

    # Extra: report Min/Max K rule coverage (Phase 48 P1)
    print()
    print("-" * 70)
    print("Phase 48 P1  —  Min/Max K rule coverage")
    print("-" * 70)
    for rule in ["EvalConst(min", "EvalConst(max", "TypeOfMinCheck", "TypeOfMaxCheck"]:
        found = rule in kloom
        status = "✓" if found else "⚠"
        if not found:
            ok = False
        print(f"{status} {rule}")

    # Extra: report persistent disable store (Phase 48 P3)
    print()
    print("-" * 70)
    print("Phase 48 P3  —  Persistent disable store")
    print("-" * 70)
    jit_rs = read_text("crates/loom-native-melior/src/jit.rs")
    has_store = "DisableStore" in jit_rs and "save" in jit_rs and "load_or_default" in jit_rs
    has_env = "LOOM_DISABLE_STORE_PATH" in jit_rs
    status = "✓" if has_store else "⚠"
    print(f"{status} DisableStore struct with save/load")
    status = "✓" if has_env else "⚠"
    print(f"{status} LOOM_DISABLE_STORE_PATH env override")

    # Extra: report corpus generator (Phase 48 P4)
    print()
    print("-" * 70)
    print("Phase 48 P4  —  Corpus generator")
    print("-" * 70)
    corpus_rs = read_text("crates/loom-fixtures/src/corpus.rs")
    has_builder = "CorpusBuilder" in corpus_rs
    has_min_max = "include_min_max" in corpus_rs
    status = "✓" if has_builder else "⚠"
    print(f"{status} CorpusBuilder")
    status = "✓" if has_min_max else "⚠"
    print(f"{status} include_min_max flag")

    print()
    print("=" * 70)
    if ok:
        print("RESULT: PASS — no critical divergences detected.")
        return 0
    else:
        print("RESULT: FAIL — some constructs are missing in one or more artifacts.")
        return 1


if __name__ == "__main__":
    sys.exit(main())
