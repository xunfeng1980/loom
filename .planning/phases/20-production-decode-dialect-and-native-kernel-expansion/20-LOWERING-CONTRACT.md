# Phase 20 Production Lowering Contract

## Scope

Phase 20 production lowering is the first native-lowering surface intended to
outlive the earlier bounded-copy spikes. It is still not host execution, not a
native artifact cache, and not a general Vortex compiler. It decides whether an
accepted Loom artifact may proceed into the `loom.decode` dialect and later MLIR
lowering.

## Trust Boundary

Production lowering starts after Loom verification. It consumes
`ArtifactVerificationReport`, not standalone `L2Core` facts and not raw layout
payloads. MLIR and native tooling live after this boundary.

## Accepted Inputs

An input may proceed only when all of the following are true:

- the artifact report status is accepted;
- artifact facts are present;
- row-count bound is present;
- a supported payload kind is present;
- L2/native facts provide a supported output schema;
- constraint status is `Discharged` or `NotRequired`.

## Discharged-Facts Rule

`ConstraintDischargeStatus::Discharged` is required when the artifact has solver
obligations. `ConstraintDischargeStatus::NotRequired` is allowed only for shapes
with no required constraints. `CollectedOnly`, `Failed`, `Unknown`, `Skipped`,
missing solver reports, missing facts, rejected reports, and unsupported reports
fail closed before dialect or MLIR text is emitted.

## Production Support Matrix

Initial production support is finite:

| Payload | Output | Status |
|---------|--------|--------|
| `LMP1 layout` | one non-null fixed-size primitive column | supported |
| `LMT1 table` | one or more non-null fixed-size primitive columns | supported |
| variable-size, dictionary, RLE, FSST, ALP, nested, nullable output | unsupported until later plans expand the matrix |

The first primitive set is Int32, Int64, Float32, and Float64.

## Diagnostics

Diagnostics are stable reviewer-facing strings. They distinguish verifier
rejection, missing artifact facts, missing row bound, constraints not
discharged, unsupported payload, unsupported type, unsupported nullability,
unsupported kernel, unsupported multi-column shape, unsupported feature, and
generic unsupported shape.

## Default vs Strict Tooling

This gate is pure `loom-core` logic and has no MLIR/LLVM dependency. Later Phase
20 validation may use optional LLVM/MLIR 22 tooling. Default workspace tests must
remain MLIR-free; strict validation may fail when compatible tools are missing.

## Non-Goals

- Host runtime ABI.
- DuckDB native execution.
- Native cache or fallback policy.
- Broad Vortex encoding coverage.
- New solver backend work.
- Checked proof objects.
